use crate::abi::rollup_abi::{CommitBatchCall, Rollup};
use dotenv::dotenv;
use ethers::providers::{Http, Provider};
use ethers::signers::Wallet;
use ethers::types::Address;
use ethers::types::Bytes;
use ethers::{abi::AbiDecode, prelude::*};
use serde::Serialize;
use std::env::var;
use std::error::Error;
use std::ops::Mul;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

#[derive(Serialize)]
pub struct ProveRequest {
    pub chunks: Vec<Vec<u64>>,
    pub rpc: String,
}

pub async fn handle_challenge() -> Result<(), Box<dyn Error>> {
    // Prepare parameter.
    dotenv().ok();
    let l1_rpc = var("L1_RPC").expect("Cannot detect L1_RPC env var");
    let l1_rollup_address = var("L1_ROLLUP").expect("Cannot detect L1_ROLLUP env var");

    let l1_provider: Provider<Http> = Provider::<Http>::try_from(l1_rpc)?;
    let l1_rollup: Rollup<Provider<Http>> = Rollup::new(
        Address::from_str(l1_rollup_address.as_str())?,
        Arc::new(l1_provider.clone()),
    );

    loop {
        std::thread::sleep(Duration::from_secs(60));

        // Step1. fetch challenge msg.
        let latest = match l1_provider.get_block_number().await {
            Ok(bn) => bn,
            Err(e) => {
                log::error!("l1_provider.get_block_number error: {:#?}", e);
                continue;
            }
        };
        let start = if latest > U64::from(100) {
            latest - U64::from(100) //100
        } else {
            latest
        };
        let filter = l1_rollup.challenge_state_filter().filter.from_block(start);

        let logs: Vec<Log> = match l1_provider.get_logs(&filter).await {
            Ok(logs) => logs,
            Err(e) => {
                log::error!("l1_provider.get_logs error: {:#?}", e);
                continue;
            }
        };
        log::debug!("l1_provider.submit_batches.get_logs.len ={:#?}", logs.len());
        let log = match logs.first() {
            Some(log) => log,
            None => {
                log::info!("no submit batches logs, latest blocknum ={:#?}", latest);
                continue;
            }
        };
        let batch_index: u64 = log.topics[0].to_low_u64_be();

        //TODO Check batch for the past 3 days(7200blocks*3 = 3 day).
        let filter = l1_rollup
            .commit_batch_filter()
            .filter
            .from_block(start - 100);
        let mut logs: Vec<Log> = match l1_provider.get_logs(&filter).await {
            Ok(logs) => logs,
            Err(e) => {
                log::error!("l1_provider.get_logs error: {:#?}", e);
                continue;
            }
        };

        let target_log = match logs
            .iter()
            .find(|log| log.topics[0].to_low_u64_be() == batch_index)
        {
            Some(log) => log,
            None => {
                continue;
            }
        };

        let hash = target_log.transaction_hash.unwrap();
        let batch_info = batch_inspect(&l1_provider, hash).await;
        if batch_info.is_none() {
            continue;
        }

        // Make a call to the Prove server.
        let request = ProveRequest {
            chunks: batch_info.unwrap(),
            rpc: String::from("example_rpc"),
        };

        let json_str = serde_json::to_string(&request).unwrap();

        let client = reqwest::blocking::Client::new();
        let url = "http://localhost/prove_block";
        let response = client
            .post(url)
            .header(
                reqwest::header::CONTENT_TYPE,
                reqwest::header::HeaderValue::from_static("application/json"),
            )
            .body(json_str)
            .send()
            .unwrap();

        println!("Response: {:?}", response.text().unwrap());
    }

    Ok(())
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
        let num_blocks = U256::from_big_endian(bs.get(..1).unwrap());
        println!("num_blocks: {:?}", num_blocks);
        for i in 0..num_blocks.as_usize() {
            let block_num =
                U256::from_big_endian(bs.get((60.mul(i) + 1)..(60.mul(i) + 1 + 8)).unwrap());
            chunk_bn.push(block_num.as_u64());
        }
        println!("chunk_bn: {:?}", chunk_bn);

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
