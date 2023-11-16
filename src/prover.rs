use crate::utils::{get_block_traces_by_number, FS_PROOF, FS_PROVE_PARAMS, FS_PROVE_SEED};
use dotenv::dotenv;
use ethers::providers::Provider;
use halo2_proofs::halo2curves::bn256::Bn256;
use halo2_proofs::poly::kzg::commitment::ParamsKZG;
use prover::aggregator::Prover as BatchProver;
use prover::config::{LayerId, LAYER4_DEGREE};
use prover::utils::chunk_trace_to_witness_block;
use prover::zkevm::Prover as ChunkProver;
use prover::{BlockTrace, ChunkHash, ChunkProof, CompressionCircuit};
use serde::{Deserialize, Serialize};
use std::env::var;
use std::fs::File;
use std::io::Write;
use std::time::Duration;
use std::{env, fs};
use std::{sync::Arc, thread};
use tokio::sync::Mutex;

// proveRequest
#[derive(Serialize, Deserialize, Debug)]
pub struct ProveRequest {
    pub batch_index: u64,
    pub chunks: Vec<Vec<u64>>,
    pub rpc: String,
}

/// Generate AggCircuitProof for block trace.
pub async fn prove_for_queue(prove_queue: Arc<Mutex<Vec<ProveRequest>>>) {
    dotenv().ok();
    env::set_var("SCROLL_PROVER_ASSETS_DIR", "./configs");
    env::set_var("CHUNK_PROTOCOL_FILENAME", "chunk.protocol");
    let mut chunk_prover = ChunkProver::from_dirs(FS_PROVE_PARAMS, "./configs");
    'task: loop {
        thread::sleep(Duration::from_millis(2000));
        log::info!("starting take request");

        // Step1. pop request from queue
        let prove_request: ProveRequest = match prove_queue.lock().await.pop() {
            Some(req) => {
                log::info!(
                    "received prove request,batch index = {:#?}, chunks len = {:#?}",
                    req.batch_index,
                    req.chunks.len()
                );
                req
            }
            None => {
                log::info!("no prove request");
                continue;
            }
        };
        // Step2. fetch trace
        let provider = match Provider::try_from(&prove_request.rpc) {
            Ok(provider) => provider,
            Err(e) => {
                log::error!("Failed to init provider: {:#?}", e);
                continue;
            }
        };
        let chunk_traces = match get_chunk_traces(&prove_request, provider).await {
            Some(chunk_traces) => chunk_traces,
            None => continue,
        };
        if chunk_traces.is_empty() {
            continue;
        }

        fs::create_dir_all(FS_PROOF.to_string() + format!("/batch_{}", &prove_request.batch_index).as_str()).unwrap();

        // Step3. start chunk prove
        let mut chunk_hashes_proofs: Vec<(ChunkHash, ChunkProof)> = vec![];
        for (index, chunk_trace) in chunk_traces.iter().enumerate() {
            let witness_chunk = chunk_trace_to_witness_block(chunk_trace.to_vec()).unwrap();
            let chunk_info = ChunkHash::from_witness_block(&witness_chunk, false);
            log::info!(
                "starting chunk prove, batch index = {:#?},chunk index = {:#?}",
                &prove_request.batch_index,
                index
            );
            let chunk_proof: ChunkProof =
                match chunk_prover.gen_chunk_proof(chunk_trace.to_vec(), None, None, Some(FS_PROOF)) {
                    Ok(proof) => {
                        log::info!("chunk prove result is: {:#?}", proof);
                        proof
                    }
                    Err(e) => {
                        log::error!("chunk prove err: {:#?}", e);
                        continue 'task;
                    }
                };
            //save chunk.protocol
            let protocol = &chunk_proof.protocol;
            let mut params_file = File::create("configs/chunk.protocol").unwrap();
            params_file.write_all(&protocol[..]).unwrap();

            chunk_hashes_proofs.push((chunk_info, chunk_proof));
        }

        if chunk_hashes_proofs.len() != chunk_traces.len() {
            log::error!("chunk proofs len err");
            continue;
        }

        // Step4. start batch prove
        log::info!("starting batch prove, batch index = {:#?}", &prove_request.batch_index);
        let mut batch_prover = BatchProver::from_dirs(FS_PROVE_PARAMS, "./configs");
        let batch_proof = batch_prover.gen_agg_evm_proof(chunk_hashes_proofs, None, Some(FS_PROOF));
        match batch_proof {
            Ok(proof) => {
                // log::info!("batch prove result is: {:#?}", proof);
                log::info!("batch prove complate");

                // batch_prover.inner.params(26);
                // let params: ParamsKZG<Bn256> = prover::utils::load_params("params_dir", 26, None).unwrap();

                // params
                // batch_prover
                log::info!("starting generate evm verifier");
                let verifier = prover::common::Verifier::<CompressionCircuit>::from_params(
                    batch_prover.inner.params(*LAYER4_DEGREE).clone(),
                    &batch_prover.get_vk().unwrap(),
                );

                let instances = proof.clone().proof_to_verify().instances();
                let num_instances: Vec<usize> = instances.iter().map(|l| l.len()).collect();

                let evm_proof = prover::EvmProof::new(
                    proof.clone().proof_to_verify().proof().to_vec(),
                    &proof.proof_to_verify().instances(),
                    num_instances,
                    batch_prover.inner.pk(LayerId::Layer4.id()),
                );
                fs::create_dir_all("evm_verifier").unwrap();
                verifier.evm_verify(&evm_proof.unwrap(), Some("evm_verifier"));
                log::info!("generate evm verifier complate");
            }
            Err(e) => log::error!("batch prove err: {:#?}", e),
        }
    }
}

async fn get_chunk_traces(
    prove_request: &ProveRequest,
    provider: Provider<ethers::providers::Http>,
) -> Option<Vec<Vec<BlockTrace>>> {
    let mut chunk_traces: Vec<Vec<BlockTrace>> = vec![];
    for chunk in &prove_request.chunks {
        let chunk_trace = match get_block_traces_by_number(&provider, &chunk).await {
            Some(traces) => traces,
            None => {
                log::error!("No trace obtained for batch: {:#?}", prove_request.batch_index);
                return None;
            }
        };
        if chunk.len() != chunk_trace.len() {
            log::error!(
                "chunk_trace.len not expect, batch index = {:#?}",
                prove_request.batch_index
            );
            return None;
        }
        chunk_traces.push(chunk_trace)
    }
    Some(chunk_traces)
}

#[tokio::test]
async fn test() {
    let protocol: Vec<u8> = vec![1, 2, 3, 4];
    std::fs::create_dir_all("configs").unwrap();
    let mut params_file = File::create("configs/chunk.protocol").unwrap();
    params_file.write_all(&protocol[..]).unwrap();
}
