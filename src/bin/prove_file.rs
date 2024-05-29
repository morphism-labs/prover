use dotenv::dotenv;
use zkevm_prover::{prover::prove_for_file, utils::read_env_var};

#[tokio::main]
async fn main() {
    dotenv().ok();
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();

    let batch_index = read_env_var("PROVE_BATCH_INDEX", 101);

    prove_for_file(batch_index).await;
}
