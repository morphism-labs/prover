use std::time::Duration;
use std::{sync::Arc, thread};
use tokio::sync::Mutex;
use ethers::providers::Provider;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use zkevm::prover::Prover;
use crate::utils::{get_block_traces_by_number, FS_PROOF, FS_PROVE_PARAMS, FS_PROVE_SEED};

// proveRequest
#[derive(Serialize, Deserialize, Debug)]
pub struct ProveRequest {
    pub block_num: u64,
    pub rpc: String,
}

/// Generate AggCircuitProof for block trace.
pub async fn prove_for_queue(prove_queue: Arc<Mutex<Vec<ProveRequest>>>) {
    //Create prover
    let mut prover = Prover::from_fpath(FS_PROVE_PARAMS, FS_PROVE_SEED);
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
            None => continue,
        };

        // Step3. start prove
        log::info!("start prove, block num is: {:#?}", prove_request.block_num);
        let proof: zkevm::prover::AggCircuitProof =
            match prover.create_agg_circuit_proof_batch(block_traces.as_slice()) {
                Ok(proof) => {
                    log::info!("the prove result is: {:#?}", proof);
                    proof
                }
                Err(e) => {
                    log::error!("prove err: {:#?}", e);
                    continue;
                }
            };
        log::info!("end prove, block num is: {:#?}", prove_request.block_num);

        // Step4. save proof
        let mut proof_path =
            PathBuf::from(FS_PROOF).join(format!("agg-proof#block#{}", prove_request.block_num));
        proof.write_to_dir(&mut proof_path);
    }
}
