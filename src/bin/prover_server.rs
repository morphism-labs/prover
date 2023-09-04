use env_logger::Env;
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

pub struct BaseResult {
    pub error_msg: String,
    pub error_code: String,
    pub result_value: String,
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
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
    fs::create_dir_all(FS_PROOF).unwrap();
    let queue: Arc<Mutex<Vec<ProveRequest>>> = Arc::new(Mutex::new(Vec::new()));

    // Step2. start mng
    let task_queue: Arc<Mutex<Vec<ProveRequest>>> = Arc::clone(&queue);
    tokio::spawn(async {
        let service = Router::new()
            .route("/prove_block", post(add_pending_req))
            .route("/query_proof", post(query_proof))
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
async fn add_pending_req(
    Extension(queue): Extension<Arc<Mutex<Vec<ProveRequest>>>>,
    param: String,
) -> String {
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
    if prove_request.block_num == 0 {
        return String::from("block_num should be greater than 0");
    }

    // Verify RPC URL format
    if !prove_request.rpc.starts_with("http://") {
        return String::from("invalid rpc url");
    }

    let proof = query_proof(prove_request.block_num.to_string()).await;
    if proof.is_empty() {
        return String::from("there are already proven results");
    }
    // Add request to queue
    queue.lock().await.push(prove_request);

    String::from("add task success")
}

// Async function to query proof data for a given block number.
// It reads the proof directory and finds the file matching the block number.
// The file contents are read into a String which is returned.
async fn query_proof(block_num: String) -> String {
    let fs: Result<fs::ReadDir, std::io::Error> = fs::read_dir(FS_PROOF);
    let mut data = String::new();
    for entry in fs.unwrap() {
        let path = entry.unwrap().path();
        if path
            .to_str()
            .unwrap()
            .contains(format!("block#{}", block_num).as_str())
        {
            let mut file = fs::File::open(path).unwrap();
            file.read_to_string(&mut data).unwrap();
        }
    }
    return data;
}

// Async function to check queue status.
// Locks queue and returns length > 0 ? "not empty" : "empty"
async fn query_status(Extension(queue): Extension<Arc<Mutex<Vec<ProveRequest>>>>) -> String {
    match queue.lock().await.len() {
        0 => String::from("queue empty"),
        _ => String::from("queue not empty"),
    }
}

#[tokio::test]
async fn test() {
    let request = ProveRequest {
        block_num: 4,
        rpc: String::from("127.0.0.1:8569"),
    };
    let info = serde_json::to_string(&request);
    println!("{:?}", info.unwrap());
}
