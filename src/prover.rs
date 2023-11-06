use crate::utils::{get_block_traces_by_number, FS_PROOF, FS_PROVE_PARAMS, FS_PROVE_SEED};
use ethers::providers::Provider;
use prover::zkevm::Prover;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use std::{sync::Arc, thread};
use tokio::sync::Mutex;

// proveRequest
#[derive(Serialize, Deserialize, Debug)]
pub struct ProveRequest {
    pub block_num: u64,
    pub rpc: String,
}

/// Generate AggCircuitProof for block trace.
pub async fn prove_for_queue(prove_queue: Arc<Mutex<Vec<ProveRequest>>>) {
    //Create prover
    let mut prover = Prover::from_dirs(FS_PROVE_PARAMS, FS_PROVE_SEED);
    loop {
        thread::sleep(Duration::from_millis(2000));

        // Step1. pop request from queue
        let prove_request = match prove_queue.lock().await.pop() {
            Some(req) => req,
            None => continue,
        };
        // Step2. fetch trace
        let provider = match Provider::try_from(prove_request.rpc) {
            Ok(provider) => provider,
            Err(e) => {
                log::error!("Failed to init provider: {:#?}", e);
                continue;
            }
        };
        let block_traces = match get_block_traces_by_number(
            &provider,
            prove_request.block_num,
            prove_request.block_num + 1,
        )
        .await
        {
            Some(traces) => traces,
            None => {
                log::info!(
                    "No trace obtained for block: {:#?}",
                    prove_request.block_num
                );
                continue;
            }
        };

        //TODO chunk_size
        for trace_trunk in block_traces.chunks(10) {
            // Step3. start prove
            log::info!("start prove, block num is: {:#?}", prove_request.block_num);
            match prover.gen_chunk_proof(trace_trunk.to_vec(), None, None, Some(FS_PROOF)) {
                Ok(proof) => {
                    log::info!("chunk prove result is: {:#?}", proof);
                }
                Err(e) => {
                    log::error!("chunk prove err: {:#?}", e);
                    continue;
                }
            };
            log::info!(
                "end chunk prove, block num is: {:#?}",
                prove_request.block_num
            );
        }
    }
}
