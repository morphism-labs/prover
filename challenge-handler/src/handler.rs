use crate::abi::rollup_abi::{CommitBatchCall, Rollup};
use dotenv::dotenv;
use ethers::providers::{Http, Provider};
use ethers::signers::Wallet;
use ethers::types::Address;
use ethers::types::Bytes;
use ethers::{abi::AbiDecode, prelude::*};
use serde::{Deserialize, Serialize};
use std::env::var;
use std::error::Error;
use std::ops::Mul;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

#[derive(Serialize)]
pub struct ProveRequest {
    pub batch_index: u64,
    pub chunks: Vec<Vec<u64>>,
    pub rpc: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ProveResult {
    pub error_msg: String,
    pub error_code: String,
    pub proof_data: String,
    pub pi_data: String,
}

pub async fn handle_challenge() -> Result<(), Box<dyn Error>> {
    // Prepare parameter.
    dotenv().ok();
    let l1_rpc = var("L1_RPC").expect("Cannot detect L1_RPC env var");
    let l1_rollup_address = var("L1_ROLLUP").expect("Cannot detect L1_ROLLUP env var");

    let l1_provider: Provider<Http> = Provider::<Http>::try_from(l1_rpc)?;
    let l1_rollup: Rollup<Provider<Http>> = Rollup::new(Address::from_str(l1_rollup_address.as_str())?, Arc::new(l1_provider.clone()));

    handle_with_zk(l1_provider, l1_rollup).await;

    Ok(())
}

async fn handle_with_zk(l1_provider: Provider<Http>, l1_rollup: Rollup<Provider<Http>>) {
    let l2_rpc = var("L2_RPC").expect("Cannot detect L2_RPC env var");
    let prover_rpc = var("PROVER_RPC").expect("Cannot detect PROVER_RPC env var");

    loop {
        std::thread::sleep(Duration::from_secs(60));

        // Step1. fetch latest blocknum.
        let latest = match l1_provider.get_block_number().await {
            Ok(bn) => bn,
            Err(e) => {
                log::error!("L1 provider.get_block_number error: {:#?}", e);
                continue;
            }
        };
        // Step2. detecte challenge event.
        let batch_index = match detecte_challenge_event(latest, &l1_rollup, &l1_provider).await {
            Some(value) => value,
            None => continue,
        };
        log::warn!("Challenge event detected, batch index is: {:#?}", batch_index);

        //Step3. query challenged batch for the past 3 days(7200blocks*3 = 3 day).
        let hash = match query_challenged_batch(latest, &l1_rollup, batch_index, &l1_provider).await {
            Some(value) => value,
            None => continue,
        };
        let batch_info = match batch_inspect(&l1_provider, hash).await {
            Some(batch) => batch,
            None => continue,
        };
        if batch_info.is_empty() {
            continue;
        }

        // Make a call to the Prove server.
        let request = ProveRequest {
            batch_index: batch_index,
            chunks: batch_info.clone(),
            rpc: l2_rpc.to_owned(),
        };

        let client = reqwest::blocking::Client::new();
        let url = prover_rpc.to_owned() + "/prove_block";
        let response = client
            .post(url)
            .header(
                reqwest::header::CONTENT_TYPE,
                reqwest::header::HeaderValue::from_static("application/json"),
            )
            .body(serde_json::to_string(&request).unwrap())
            .send()
            .unwrap();

        println!("Response: {:?}", response.text().unwrap());
        log::info!("Successfully submitted proof task, waiting for proof to be generated");

        std::thread::sleep(Duration::from_secs((4200 * batch_info.len() + 1800) as u64)); //chunk_prove_time =1h 10min，batch_prove_time = 24min；
        prove_state(batch_index, prover_rpc.to_owned(), &l1_rollup).await;
    }
}

async fn prove_state(batch_index: u64, prover_rpc: String, l1_rollup: &Rollup<Provider<Http>>) -> bool {
    let url = prover_rpc + "/query_proof";
    loop {
        std::thread::sleep(Duration::from_secs(300));

        // Make a call to the Prove server.
        let client = reqwest::blocking::Client::new();
        let response = client
            .post(url.to_owned())
            .header(
                reqwest::header::CONTENT_TYPE,
                reqwest::header::HeaderValue::from_static("application/json"),
            )
            .body(batch_index.to_string())
            .send();
        let rt = match response {
            Ok(x) => x.text(),
            Err(e) => {
                log::error!("query proof of {:#?} error: {:#?}", batch_index, e);
                continue;
            }
        };

        let rt_text = match rt {
            Ok(x) => x,
            Err(e) => {
                log::error!("query proof bytes of {:#?} error: {:#?}", batch_index, e);
                continue;
            }
        };

        let prove_result: ProveResult = match serde_json::from_str(rt_text.as_str()) {
            Ok(pr) => pr,
            Err(_) => {
                log::error!("deserialize prove_result failed, batch index = {:#?}", batch_index);
                return false;
            }
        };

        if prove_result.pi_data.is_empty() || prove_result.proof_data.is_empty() {
            log::info!("query proof of {:#?}, pi_data or  proof_data is empty", batch_index);
            continue;
        }

        // println!("Response: {:?}", response.text().unwrap());
        let aggr_proof = Bytes::decode(prove_result.proof_data).unwrap();
        let tx = l1_rollup.prove_state(batch_index, aggr_proof);
        let rt = tx.send().await;
        match rt {
            Ok(info) => {
                log::info!("tx of prove_state has been sent: {:#?}", info.tx_hash());
                return true;
            }
            Err(e) => log::error!("send tx of prove_state error: {:#?}", e),
        }
    }
}

async fn query_challenged_batch(latest: U64, l1_rollup: &Rollup<Provider<Http>>, batch_index: u64, l1_provider: &Provider<Http>) -> Option<TxHash> {
    let start = if latest > U64::from(7200 * 3) {
        latest - U64::from(7200 * 3)
    } else {
        U64::from(1)
    };
    let filter = l1_rollup.commit_batch_filter().filter.from_block(start).topic1(U256::from(batch_index));
    let logs: Vec<Log> = match l1_provider.get_logs(&filter).await {
        Ok(logs) => logs,
        Err(e) => {
            log::error!("l1_rollup.commit_batch.get_logs error: {:#?}", e);
            return None;
        }
    };
    if logs.is_empty() {
        log::warn!("no commit_batch log of {:?}, commit_batch logs is empty", batch_index);
        return None;
    }
    let target_log = match logs.iter().find(|log| log.topics[1].to_low_u64_be() == batch_index) {
        Some(log) => log,
        None => {
            log::warn!("no commit_batch log of {:?}", batch_index);
            return None;
        }
    };
    let hash = target_log.transaction_hash.unwrap();
    Some(hash)
}

async fn detecte_challenge_event(latest: U64, l1_rollup: &Rollup<Provider<Http>>, l1_provider: &Provider<Http>) -> Option<u64> {
    let start = if latest > U64::from(100) {
        latest - U64::from(100) //100
    } else {
        U64::from(1)
    };
    let filter = l1_rollup.challenge_state_filter().filter.from_block(start);
    let logs: Vec<Log> = match l1_provider.get_logs(&filter).await {
        Ok(logs) => logs,
        Err(e) => {
            log::error!("l1_rollup.challenge_state.get_logs error: {:#?}", e);
            return None;
        }
    };
    log::debug!("l1_rollup.challenge_state.get_logs.len = {:#?}", logs.len());
    let log = match logs.first() {
        Some(log) => log,
        None => {
            log::info!("no challenge state logs, latest blocknum = {:#?}", latest);
            return None;
        }
    };
    let batch_index: u64 = log.topics[1].to_low_u64_be();

    let batch_in_challenge: bool = match l1_rollup.batch_in_challenge(U256::from(batch_index)).await {
        Ok(x) => x,
        Err(e) => {
            log::info!("query l1_rollup.batch_in_challenge error, batch index = {:#?}, {:#?}", batch_index, e);
            return None;
        }
    };

    if batch_in_challenge == false {
        log::info!("batch not in challenge, batch index = {:#?}", batch_index);
        return None;
    }

    Some(batch_index)
}

async fn batch_inspect(l1_provider: &Provider<Http>, hash: TxHash) -> Option<Vec<Vec<u64>>> {
    //Step1.  Get transaction
    let result = l1_provider.get_transaction(hash).await;
    let tx = match result {
        Ok(Some(tx)) => tx,
        Ok(None) => {
            log::error!("l1_provider.get_transaction is none");
            return None;
        }
        Err(e) => {
            log::error!("l1_provider.get_transaction err: {:#?}", e);
            return None;
        }
    };

    //Step2. Parse transaction data
    let data = tx.input;
    if data.is_empty() {
        log::warn!("tx.input is empty, tx_hash =  {:#?}", hash);
        return None;
    }
    let param = if let Ok(_param) = CommitBatchCall::decode(&data) {
        _param
    } else {
        log::error!("decode tx.input error, tx_hash =  {:#?}", hash);
        return None;
    };
    let chunks: Vec<Bytes> = param.batch_data.chunks;
    if let Some(value) = decode_chunks(chunks) {
        return Some(value);
    }
    Some(vec![])
}

fn decode_chunks(chunks: Vec<Bytes>) -> Option<Vec<Vec<u64>>> {
    if chunks.is_empty() {
        return None;
    }

    let mut chunk_vec: Vec<Vec<u64>> = vec![];
    for chunk in chunks.iter() {
        //&ethers::types::Bytes
        let mut chunk_bn: Vec<u64> = vec![];
        let bs: &[u8] = chunk;

        // decode blocks from chunk
        // |   1 byte   | 60 bytes | ... | 60 bytes |
        // | num blocks |  block 1 | ... |  block n |
        let num_blocks = U256::from_big_endian(bs.get(..1).unwrap());
        for i in 0..num_blocks.as_usize() {
            let block_num = U256::from_big_endian(bs.get((60.mul(i) + 1)..(60.mul(i) + 1 + 8)).unwrap());
            chunk_bn.push(block_num.as_u64());
        }
        log::info!("chunk_bn: {:#?}", chunk_bn);

        chunk_vec.push(chunk_bn);
    }

    return Some(chunk_vec);
}

#[tokio::test]
async fn test_decode_chunks() {
    use std::fs::File;
    use std::io::Read;
    let mut file = File::open("./src/input.data").unwrap();
    let mut contents = String::new();
    file.read_to_string(&mut contents).unwrap();
    let input = Bytes::from_str(contents.as_str()).unwrap();

    let param = CommitBatchCall::decode(&input).unwrap();
    let chunks: Vec<Bytes> = param.batch_data.chunks;
    let rt = decode_chunks(chunks).unwrap();
    assert!(rt.len() == 11);
    assert!(rt.get(3).unwrap().len() == 2);
}

#[tokio::test]
async fn test_commit_filter() {
    let l1_provider: Provider<Http> = Provider::<Http>::try_from("https://eth-mainnet.g.alchemy.com/v2/oGHoxvEj_29sdJlOjKMPMHvmqS8gYmLX").unwrap();
    let l1_rollup: Rollup<Provider<Http>> = Rollup::new(
        Address::from_str("0xa13BAF47339d63B743e7Da8741db5456DAc1E556").unwrap(),
        Arc::new(l1_provider.clone()),
    );

    let latest = l1_provider.get_block_number().await.unwrap();
    let filter = l1_rollup.commit_batch_filter().filter.from_block(18548027).topic1(U256::from(20856));
    let logs: Vec<Log> = l1_provider.get_logs(&filter).await.unwrap();

    println!("{:?}", logs.len());
    println!("{:?}", logs.first().unwrap().topics[0]);
}
