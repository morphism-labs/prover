use env_logger::Env;
use ethers::providers::{Http, Provider};
use rand::SeedableRng;
use rand_xorshift::XorShiftRng;
use std::env::var;
use std::path::PathBuf;
use std::str::FromStr;
use std::fs;
use zkevm::{
    circuit::{AGG_DEGREE, DEGREE},
    io,
    prover::Prover,
    utils::{load_or_create_params, load_or_create_seed},
};
use zkevm_prover::utils;


// Used to generate poof for specified blocks in the development environment.
// It will read the cmd parameters and execute the following process.
// Main flow:
// 1. Init env 
// 2. Fetch block traces
// 3. Create prover 
// 4. Generate proof
// 5. Write proof & verifier
#[tokio::main]
async fn main() {
    // Step1. prepare param
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
    let block_num: u64 = var("PROVERD_BLOCK_NUM")
        .expect("PROVERD_BLOCK_NUM env var")
        .parse()
        .expect("Cannot parse PROVERD_BLOCK_NUM env var");
    let rpc_url: String = var("PROVERD_RPC_URL")
        .expect("PROVERD_RPC_URL env var")
        .parse()
        .expect("Cannot parse PROVERD_RPC_URL env var");
    let params_path: String = var("PARAMS_PATH")
        .expect("PARAMS_PATH env var")
        .parse()
        .expect("Cannot parse PARAMS_PATH env var");

    let provider = Provider::<Http>::try_from(rpc_url)
        .expect("failed to initialize ethers Provider");

    // Step 2. fetch block trace
    let block_traces = utils::get_block_traces_by_number(&provider, block_num, block_num + 1)
        .await
        .unwrap();

    log::info!("block_traces_len is: {:#?}", block_traces.len());

    log::info!("block_traces_chain_id is: {:#?}", block_traces[0].chain_id);

    // Step 3. create prover
    let mut prover = create_prover(params_path);

    // Step 4. start prove
    let block_trace_array = block_traces.as_slice();
    let result = prover.create_agg_circuit_proof_batch(block_trace_array);
    match result {
        Ok(proof) => {
            log::info!("prove result is: {:#?}", proof);
            // Save proof
            let mut proof_path = PathBuf::from("./proof").join("agg.proof");
            fs::create_dir_all(&proof_path).unwrap();
            proof.write_to_dir(&mut proof_path);
            // Save verify contract
            let solidity = prover.create_solidity_verifier(&proof);
            log::info!("verify solidity is: {:#?}", solidity);

            let mut folder = PathBuf::from_str("./verifier").unwrap();
            io::write_verify_circuit_solidity(&mut folder, &Vec::<u8>::from(solidity.as_bytes()))
        }
        Err(e) => {
            log::info!("prove err: {:#?}", e);
        }
    };
}

/**
 * Create prover of zkevm
 */
fn create_prover(params_path: String) -> Prover {

    let params = load_or_create_params(params_path.as_str(), *DEGREE)
        .expect("failed to load or create kzg params");
    let agg_params = load_or_create_params(params_path.as_str(), *AGG_DEGREE)
        .expect("failed to load or create agg-kzg params");
    let seed = load_or_create_seed("./prove_seed").expect("failed to load or create seed");
    let rng = XorShiftRng::from_seed(seed);

    Prover::from_params_and_rng(params, agg_params, rng)
}
