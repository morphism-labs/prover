use axum::{routing::get, Router};
use challenge_handler::{
    handler,
    metrics::{METRICS, REGISTRY},
};
use env_logger::Env;
use prometheus::{Encoder, TextEncoder};
use tower_http::trace::TraceLayer;
use dotenv::dotenv;

#[tokio::main]
async fn main() {
    // Initialize logger.
    dotenv().ok();
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
    log::info!("Starting challenge handler...");

    // prometheus.
    register_metrics();
    tokio::spawn(async move {
        let metrics = Router::new().route("/metrics", get(handle_metrics)).layer(TraceLayer::new_for_http());
        axum::Server::bind(&"0.0.0.0:6021".parse().unwrap())
            .serve(metrics.into_make_service())
            .await
            .unwrap();
    });

    // Start challenge handler.
    let result = handler::handle_challenge().await;

    // Handle result.
    match result {
        Ok(()) => (),
        Err(e) => {
            log::error!("challenge handler exec error: {:#?}", e);
        }
    }
}

fn register_metrics() {
    // detected batch index.
    REGISTRY.register(Box::new(METRICS.detected_batch_index.clone())).unwrap();
    // chunks len.
    REGISTRY.register(Box::new(METRICS.chunks_len.clone())).unwrap();
    // prover status.
    REGISTRY.register(Box::new(METRICS.prover_status.clone())).unwrap();
}

async fn handle_metrics() -> String {
    let mut buffer = Vec::new();
    let encoder = TextEncoder::new();

    // Gather the metrics.
    let mut metric_families = REGISTRY.gather();
    metric_families.extend(prometheus::gather());

    // Encode metrics to send.
    match encoder.encode(&metric_families, &mut buffer) {
        Ok(()) => {
            let output = String::from_utf8(buffer.clone()).unwrap();
            return output;
        }
        Err(e) => {
            log::error!("encode metrics error: {:#?}", e);
            return String::from("");
        }
    }
}

#[test]
fn test() {
    let mut data: Vec<i32> = vec![1];
    let value = data.first();
    println!("value: {:?}", value);
    println!("{:?}", data.len());

    let value1 = data.pop();
    println!("value: {:?}", value1);
    println!("{:?}", data.len());
}
