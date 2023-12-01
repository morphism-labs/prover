use challenge_handler::abi::rollup_abi::Rollup;
use dotenv::dotenv;
use env_logger::Env;
use ethers::prelude::*;
use ethers::signers::Wallet;
use std::env::var;
use std::error::Error;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

/**
 * Search for the latest batch to challenge
 */
#[tokio::main]
pub async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    dotenv().ok();
    let l1_rpc = var("L1_RPC").expect("Cannot detect L1_RPC env var");
    let l1_rollup_address = var("L1_ROLLUP").expect("Cannot detect L1_ROLLUP env var");
    let private_key = var("CHALLENGER_PRIVATEKEY").expect("Cannot detect CHALLENGER_PRIVATEKEY env var");
    let challenge: bool = var("CHALLENGE")
        .expect("Cannot detect CHALLENGE env var")
        .parse()
        .expect("Cannot parse CHALLENGE env var");
    log::info!("starting... challenge = {:#?}", challenge);

    let l1_provider: Provider<Http> = Provider::<Http>::try_from(l1_rpc)?;
    let l1_signer = Arc::new(SignerMiddleware::new(
        l1_provider.clone(),
        Wallet::from_str(private_key.as_str())
            .unwrap()
            .with_chain_id(l1_provider.get_chainid().await.unwrap().as_u64()),
    ));
    let l1_rollup = Rollup::new(Address::from_str(l1_rollup_address.as_str())?, l1_signer);

    let latest = match l1_provider.get_block_number().await {
        Ok(bn) => bn,
        Err(e) => {
            log::error!("L1 provider.get_block_number error: {:#?}", e);
            return Ok(());
        }
    };
    log::info!("latest blocknum = {:#?}", latest);

    let start = if latest > U64::from(200) {
        latest - U64::from(200)
    } else {
        U64::from(1)
    };
    let filter = l1_rollup.commit_batch_filter().filter.from_block(start).address(l1_rollup.address());
    let logs: Vec<Log> = match l1_provider.get_logs(&filter).await {
        Ok(logs) => logs,
        Err(e) => {
            log::error!("l1_rollup.commit_batch.get_logs error: {:#?}", e);
            return Ok(());
        }
    };

    if logs.is_empty() {
        log::error!("no commit_batch log");
        return Ok(());
    }
    let batch_index = match logs.last() {
        Some(log) => log.topics[1].to_low_u64_be(),
        None => {
            log::error!("find commit_batch log error");
            return Ok(());
        }
    };
    log::info!("latest batch index = {:#?}", batch_index);

    if challenge == false {
        log::info!("No need for challenge");
        return Ok(());
    }

    let tx = l1_rollup.challenge_state(batch_index);
    let rt = tx.send().await;
    let pending_tx = match rt {
        Ok(pending_tx) => {
            log::info!("tx of challenge_state has been sent: {:#?}", pending_tx.tx_hash());
            pending_tx
        }
        Err(e) => {
            log::error!("send tx of challenge_state error: {:#?}", e);
            return Ok(());
        }
    };

    std::thread::sleep(Duration::from_secs(16));
    let receipt = l1_provider.get_transaction_receipt(pending_tx.tx_hash()).await.unwrap();

    match receipt {
        Some(tr) => {
            match tr.status.unwrap().as_u64() {
                1 => log::info!("challenge_state receipt success: {:#?}", tr),
                _ => {
                    log::error!("challenge_state receipt fail: {:#?}", tr);
                }
            };
        }
        // Maybe still pending
        None => {
            log::info!("challenge_state receipt pending");
        }
    }
    Ok(())
}
