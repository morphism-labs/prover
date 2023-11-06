use ethers::providers::{Http, Provider};
use prover::BlockTrace;

pub const FS_PROVE_PARAMS: &'static str = "prove_params";
pub const FS_PROVE_SEED: &'static str = "prove_seed";
pub const FS_PROOF: &'static str = "proof";


// Fetches block traces by provider 
pub async fn get_block_traces_by_number(
    provider: &Provider<Http>,
    block_start: u64,
    block_end: u64,
) -> Option<Vec<BlockTrace>> {
    let mut block_traces: Vec<BlockTrace> = Vec::new();
    for i in block_start..block_end {
        log::info!("zkevm-prover: requesting trace of block {i}");
        let result = provider
            .request("scroll_getBlockTraceByNumberOrHash", [format!("{i:#x}")])
            .await;
        match result {
            Ok(trace) => block_traces.push(trace),
            Err(e) => log::error!("zkevm-prover: requesting trace error: {e}"),
        }
    }
    Some(block_traces)
}
