use crate::utils::{get_block_traces_by_number, FS_PROOF, FS_PROVE_PARAMS, FS_PROVE_SEED};
use ethers::providers::Provider;
use prover::aggregator::Prover as BatchProver;
use prover::utils::chunk_trace_to_witness_block;
use prover::zkevm::Prover as ChunkProver;
use prover::{ChunkHash, ChunkProof};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs::File;
use std::io::Write;
use std::time::Duration;
use std::{sync::Arc, thread};
use tokio::sync::Mutex;

// proveRequest
#[derive(Serialize, Deserialize, Debug)]
pub struct ProveRequest {
    pub block_num_start: u64,
    pub block_num_end: u64,
    pub rpc: String,
}

/// Generate AggCircuitProof for block trace.
pub async fn prove_for_queue(prove_queue: Arc<Mutex<Vec<ProveRequest>>>) {
    //Create prover
    env::set_var("SCROLL_PROVER_ASSETS_DIR", "./configs");
    env::set_var("CHUNK_PROTOCOL_FILENAME", "chunk.protocol");
    let mut chunk_prover = ChunkProver::from_dirs(FS_PROVE_PARAMS, "./configs");
    loop {
        thread::sleep(Duration::from_millis(2000));
        log::info!("starting take request");

        // Step1. pop request from queue
        let prove_request: ProveRequest = match prove_queue.lock().await.pop() {
            Some(req) => {
                log::info!("received prove request");
                req
            }
            None => {
                log::info!("no prove request");
                continue;
            }
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
            prove_request.block_num_start,
            prove_request.block_num_end,
        )
        .await
        {
            Some(traces) => traces,
            None => {
                log::info!(
                    "No trace obtained for block: {:#?}",
                    prove_request.block_num_start
                );
                continue;
            }
        };
        if block_traces.is_empty(){
            continue;
        }

        //TODO chunk_size
        let mut chunk_hashes_proofs: Vec<(ChunkHash, ChunkProof)> = vec![];
        log::info!("staring trunk prove");
        for trace_trunk in block_traces.chunks(1) {
            let witness_chunk = chunk_trace_to_witness_block(trace_trunk.to_vec()).unwrap();
            let chunk_info = ChunkHash::from_witness_block(&witness_chunk, false);
            // Step3. start prove
            log::info!(
                "start prove, block num is: {:#?}",
                prove_request.block_num_start
            );
            let chunk_proof: ChunkProof = match chunk_prover.gen_chunk_proof(
                trace_trunk.to_vec(),
                None,
                None,
                Some(FS_PROOF),
            ) {
                Ok(proof) => {
                    log::info!("chunk prove result is: {:#?}", proof);
                    proof
                }
                Err(e) => {
                    log::error!("chunk prove err: {:#?}", e);
                    continue;
                }
            };
            //save chunk.protocol
            let protocol = &chunk_proof.protocol;
            let mut params_file = File::create("configs/chunk.protocol").unwrap();
            params_file.write_all(&protocol[..]).unwrap();

            chunk_hashes_proofs.push((chunk_info, chunk_proof));
            log::info!(
                "end chunk prove, block num is: {:#?}",
                prove_request.block_num_start
            );
        }

        // if chunk_hashes_proofs.len() != 2 {
        //     log::error!("chunk proof len err");
        //     continue;
        // }

        log::info!("staring batch prove");
        let mut batch_prover = BatchProver::from_dirs(FS_PROVE_PARAMS, "./configs");
        let batch_proof = batch_prover.gen_agg_evm_proof(chunk_hashes_proofs, None, Some(FS_PROOF));
        match batch_proof {
            Ok(proof) => {
                log::info!("batch prove result is: {:#?}", proof);
            }
            Err(e) => log::error!("batch prove err: {:#?}", e),
        }
    }
}

#[tokio::test]
async fn test() {
    let protocol: Vec<u8> = vec![1, 2, 3, 4];
    std::fs::create_dir_all("configs").unwrap();
    let mut params_file = File::create("configs/chunk.protocol").unwrap();
    params_file.write_all(&protocol[..]).unwrap();
}
