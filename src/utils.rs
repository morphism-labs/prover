use std::str::FromStr;

use ethers::providers::{Http, Provider};
use once_cell::sync::Lazy;
use prover::BlockTrace;

pub static PROVER_PROOF_DIR: Lazy<String> = Lazy::new(|| read_env_var("PROVER_PROOF_DIR", "./proof".to_string()));
pub static PROVER_PARAMS_DIR: Lazy<String> =
    Lazy::new(|| read_env_var("PROVER_PARAMS_DIR", "./prove_params".to_string()));
pub static SCROLL_PROVER_ASSETS_DIR: Lazy<String> =
    Lazy::new(|| read_env_var("SCROLL_PROVER_ASSETS_DIR", "./prove_params".to_string()));
pub static PROVER_L2_RPC: Lazy<String> =
    Lazy::new(|| read_env_var("PROVER_L2_RPC", "./prove_params".to_string()));

// Fetches block traces by provider
pub async fn get_block_traces_by_number(provider: &Provider<Http>, block_nums: &Vec<u64>) -> Option<Vec<BlockTrace>> {
    let mut block_traces: Vec<BlockTrace> = Vec::new();
    for block_num in block_nums {
        log::info!("zkevm-prover: requesting trace of block {block_num}");
        let result = provider
            .request("morph_getBlockTraceByNumberOrHash", [format!("{block_num:#x}")])
            .await;

        match result {
            Ok(trace) => block_traces.push(trace),
            Err(e) => {
                log::error!("zkevm-prover: requesting trace error: {e}");
                return None;
            }
        }
    }
    Some(block_traces)
}

pub fn read_env_var<T: Clone + FromStr>(var_name: &'static str, default: T) -> T {
    std::env::var(var_name)
        .map(|s| s.parse::<T>().unwrap_or_else(|_| default.clone()))
        .unwrap_or(default)
}
