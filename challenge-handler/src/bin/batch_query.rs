use challenge_handler::abi::rollup_abi::Rollup;
use dotenv::dotenv;
use env_logger::Env;
use ethers::prelude::*;
use ethers::signers::Wallet;
use std::env::var;
use std::error::Error;
use std::str::FromStr;
use std::sync::Arc;

type RollupType = Rollup<SignerMiddleware<Provider<Http>, LocalWallet>>;

/**
 * Search for the latest batch to challenge
 */
#[tokio::main]
pub async fn main() -> Result<(), Box<dyn Error>> {
    // Prepare env.
    env_logger::Builder::from_env(Env::default().default_filter_or("debug")).init();
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
    let challenger_address = l1_signer.address();
    let l1_rollup: RollupType = Rollup::new(Address::from_str(l1_rollup_address.as_str())?, l1_signer);

    // Check rollup state.
    let is_challenger: bool = match l1_rollup.is_challenger(challenger_address).await {
        Ok(x) => x,
        Err(e) => {
            log::info!("query l1_rollup.is_challenger error: {:#?}", e);
            return Ok(());
        }
    };
    log::info!("address({:#?})  is_challenger: {:#?}", challenger_address, is_challenger);

    let challenger_balance = l1_provider.get_balance(challenger_address, None).await.unwrap();
    log::info!("challenger_eth_balance: {:#?}", challenger_balance);

    let finalization_period = l1_rollup.finalization_period_seconds().await?;
    let proof_window = l1_rollup.proof_window().await?;
    log::info!("finalization_period: ({:#?})  proof_window: {:#?}", finalization_period, proof_window);

    // Search for the latest batch
    let latest = match l1_provider.get_block_number().await {
        Ok(bn) => bn,
        Err(e) => {
            log::error!("L1 provider.get_block_number error: {:#?}", e);
            return Ok(());
        }
    };

    log::info!("latest blocknum = {:#?}", latest);
    let start = if latest > U64::from(1000) {
        latest - U64::from(1000)
    } else {
        U64::from(1)
    };

    let filter = l1_rollup.commit_batch_filter().filter.from_block(start).address(l1_rollup.address());
    let mut logs: Vec<Log> = match l1_provider.get_logs(&filter).await {
        Ok(logs) => logs,
        Err(e) => {
            log::error!("l1_rollup.commit_batch.get_logs error: {:#?}", e);
            return Ok(());
        }
    };

    if logs.is_empty() {
        log::error!("There have been no commit_batch logs for the last 1000 blocks.");
        return Ok(());
    }
    logs.sort_by(|a, b| a.block_number.unwrap().cmp(&b.block_number.unwrap()));
    let batch_index = match logs.last() {
        Some(log) => log.topics[1].to_low_u64_be(),
        None => {
            log::error!("find commit_batch log error");
            return Ok(());
        }
    };
    log::info!("latest batch index = {:#?}", batch_index);

    // challenge_batch = challenge_batch.max(batch_index);

    let filter = l1_rollup
        .commit_batch_filter()
        .filter
        .from_block(start)
        .topic1(U256::from(batch_index))
        .address(l1_rollup.address());
    let logs: Vec<Log> = match l1_provider.get_logs(&filter).await {
        Ok(logs) => logs,
        Err(e) => {
            log::error!("l1_rollup.commit_batch.get_logs error: {:#?}", e);
            vec![]
        }
    };

    if logs.is_empty() {
        log::error!("no commit_batch log of {:?}, commit_batch logs is empty", batch_index);
        return Ok(());
    }

    for log in logs {
        if log.topics[1].to_low_u64_be() != batch_index {
            log::error!("commit_batch batch_index missing: {:?}", batch_index);
            continue;
        }
        let tx_hash = log.transaction_hash.unwrap();
        let result = l1_provider.get_transaction(tx_hash).await.unwrap().unwrap();
        let data = result.input;
        log::info!("batch inspect: tx.input =  {:#?}", data);
    }
    Ok(())
}
