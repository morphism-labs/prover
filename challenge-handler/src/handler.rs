use crate::abi::rollup_abi::{CommitBatchCall, Rollup};
use dotenv::dotenv;
use ethers::providers::{Http, Provider};
use ethers::signers::Wallet;
use ethers::types::Address;
use ethers::{abi::AbiDecode, prelude::*};
use std::env::var;
use std::error::Error;
use std::str::FromStr;
use std::sync::Arc;

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
        // Step1. fetch latest batches and calculate overhead.
        let latest = match l1_provider.get_block_number().await {
            Ok(bn) => bn,
            Err(e) => {
                log::error!("overhead.l1_provider.get_block_number error: {:#?}", e);
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
                log::error!("overhead.l1_provider.get_logs error: {:#?}", e);
                continue;
            }
        };
        log::debug!(
            "overhead.l1_provider.submit_batches.get_logs.len ={:#?}",
            logs.len()
        );
        let topic: H256 = logs[0].topics[0];
        let batch_index: u64 = topic.to_low_u64_be();

        let filter = l1_rollup.commit_batch_filter().filter.from_block(start);
        let mut logs: Vec<Log> = match l1_provider.get_logs(&filter).await {
            Ok(logs) => logs,
            Err(e) => {
                log::error!("overhead.l1_provider.get_logs error: {:#?}", e);
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
    }

    Ok(())
}

async fn batch_inspect(l1_provider: &Provider<Http>, hash: TxHash) -> Option<Vec<(u64, u64)>> {
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
        log::warn!("overhead_inspect tx.input is empty, tx_hash =  {:#?}", hash);
        return None;
    }
    let param = if let Ok(_param) = CommitBatchCall::decode(&data) {
        _param
    } else {
        log::error!(
            "overhead_inspect decode tx.input error, tx_hash =  {:#?}",
            hash
        );
        return None;
    };
    let chunks = param.batch_data.chunks;
    if chunks.is_empty() {
        return None;
    }

    for chunk in chunks.iter() {
        //&ethers::types::Bytes
        let bs: &[u8] = chunk;
    }
    Some(vec![(1, 10)])
}
