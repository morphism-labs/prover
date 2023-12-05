use env_logger::Env;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Read;
use std::sync::Arc;
use tokio::sync::Mutex;

use axum::extract::Extension;
use axum::{routing::post, Router};

use tower_http::add_extension::AddExtensionLayer;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use zkevm_prover::prover::{prove_for_queue, ProveRequest};
use zkevm_prover::utils::FS_PROOF;

#[derive(Serialize, Deserialize, Debug)]
pub struct ProveResult {
    pub error_msg: String,
    pub error_code: String,
    pub proof_data: String,
    pub pi_data: String,
}

// Main async function to start prover service.
// 1. Initializes environment.
// 2. Spawns management server.
// 3. Start the Prover on the main thread with shared queue.
// Server handles prove requests .
// Prover consumes requests and generates proofs and save.
#[tokio::main]
async fn main() {
    // Step1. prepare environment
    env_logger::Builder::from_env(Env::default().default_filter_or("debug")).init();
    fs::create_dir_all(FS_PROOF).unwrap();
    let queue: Arc<Mutex<Vec<ProveRequest>>> = Arc::new(Mutex::new(Vec::new()));

    // Step2. start mng
    let task_queue: Arc<Mutex<Vec<ProveRequest>>> = Arc::clone(&queue);
    tokio::spawn(async {
        let service = Router::new()
            .route("/prove_batch", post(add_pending_req))
            .route("/query_proof", post(query_prove_result))
            .route("/query_status", post(query_status))
            .layer(AddExtensionLayer::new(task_queue))
            .layer(CorsLayer::permissive())
            .layer(TraceLayer::new_for_http());

        axum::Server::bind(&"127.0.0.1:3030".parse().unwrap())
            .serve(service.into_make_service())
            .await
            .unwrap();
    });

    // Step3. start prover
    let prove_queue: Arc<Mutex<Vec<ProveRequest>>> = Arc::clone(&queue);
    prove_for_queue(prove_queue).await;
}

// Add pending prove request to queue
async fn add_pending_req(Extension(queue): Extension<Arc<Mutex<Vec<ProveRequest>>>>, param: String) -> String {
    // Verify parameter is not empty
    if param.is_empty() {
        return String::from("request is empty");
    }

    // Deserialize parameter to ProveRequest type
    let prove_request: Result<ProveRequest, serde_json::Error> = serde_json::from_str(&param);

    // Handle deserialization result
    let prove_request = match prove_request {
        Ok(req) => req,
        Err(_) => return String::from("deserialize proveRequest failed"),
    };

    // Verify block number is greater than 0
    if prove_request.chunks.len() == 0 {
        return String::from("chunks is empty");
    }

    // Verify RPC URL format
    if !prove_request.rpc.starts_with("http://") && !prove_request.rpc.starts_with("https://") {
        return String::from("invalid rpc url");
    }

    if queue.lock().await.len() > 0 {
        return String::from("add prove batch fail, please waiting for the pending task to complete");
    }

    let fs: Result<fs::ReadDir, std::io::Error> = fs::read_dir(FS_PROOF);
    for entry in fs.unwrap() {
        let path = entry.unwrap().path();
        if path
            .to_str()
            .unwrap()
            .contains(format!("/batch_{}", prove_request.batch_index).as_str())
        {
            log::warn!("Prover is proving this batch: {:#?}", prove_request.batch_index);
            return String::from("Prover is proving this batch");
        }
    }

    let proof = query_proof(prove_request.batch_index.to_string()).await;
    if !proof.proof_data.is_empty() || !proof.pi_data.is_empty() {
        log::warn!("there are already proven results: {:#?}", prove_request.batch_index);
        return String::from("there are already proven results");
    }
    // Add request to queue
    queue.lock().await.push(prove_request);

    String::from("success")
}

// Async function to query proof data for a given block number.
// It reads the proof directory and finds the file matching the block number.
// The file contents are read into a String which is returned.
async fn query_prove_result(batch_index: String) -> String {
    let result = query_proof(batch_index).await;
    return serde_json::to_string(&result).unwrap();
}

async fn query_proof(batch_index: String) -> ProveResult {
    let fs: Result<fs::ReadDir, std::io::Error> = fs::read_dir(FS_PROOF);
    let mut result = ProveResult {
        error_msg: String::from(""),
        error_code: String::from(""),
        proof_data: String::from(""),
        pi_data: String::from(""),
    };
    for entry in fs.unwrap() {
        let path = entry.unwrap().path();
        if path
            .to_str()
            .unwrap()
            .contains(format!("/batch_{}", batch_index).as_str())
        {
            //pi_batch_agg.data
            let proof_path = path.join("proof_batch_agg.data");
            let mut proof_data = String::new();
            match fs::File::open(proof_path) {
                Ok(mut file) => {
                    file.read_to_string(&mut proof_data).unwrap();
                }
                Err(e) => {
                    log::error!("Failed to load proof_data: {:#?}", e);
                    result.error_msg = String::from("Failed to load proof_data");
                }
            }
            result.proof_data = proof_data;

            let pi_path = path.join("pi_batch_agg.data");
            let mut pi_data = String::new();
            match fs::File::open(pi_path) {
                Ok(mut file) => {
                    file.read_to_string(&mut pi_data).unwrap();
                }
                Err(e) => {
                    log::error!("Failed to load pi_data: {:#?}", e);
                    result.error_msg = String::from("Failed to load pi_data");
                }
            }
            result.pi_data = pi_data;
            break;
        }
    }
    return result;
}

// Async function to check queue status.
// Locks queue and returns length > 0 ? "not empty" : "empty"
async fn query_status(Extension(queue): Extension<Arc<Mutex<Vec<ProveRequest>>>>) -> String {
    match queue.lock().await.len() {
        0 => String::from("0"),
        _ => String::from("1"),
    }
}

#[tokio::test]
async fn test_query_proof() {
    let proof = query_proof("4".to_string()).await;
    println!("{:?}", proof);
}

#[tokio::test]
async fn test() {
    let request = ProveRequest {
        batch_index: 4,
        chunks: vec![vec![1], vec![2, 3]],
        rpc: String::from("127.0.0.1:8569"),
    };
    let info = serde_json::to_string(&request);
    println!("{:?}", info.unwrap());
}
