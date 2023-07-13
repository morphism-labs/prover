use clap::Parser;
use zkevm::{
    circuit::{AGG_DEGREE, DEGREE},
    utils::{load_or_create_params, load_or_create_seed},
};
use zkevm_prover::utils::{FS_PROVE_PARAMS, FS_PROVE_SEED};

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// generate params and write into file
    #[clap(short, long = "params")]
    params_path: Option<String>,
    /// generate seed and write into file
    #[clap(short, long = "seed")]
    seed_path: Option<String>,
}

// Set the kzg parameters required to run the zkevm circuit.
fn main() {
    dotenv::dotenv().ok();
    env_logger::init();

    let args = Args::parse();
    let params_path = match args.params_path {
        Some(path) => path,
        None => String::from(FS_PROVE_PARAMS),
    };
    let seed_path = match args.seed_path {
        Some(path) => path,
        None => String::from(FS_PROVE_SEED),
    };
    
    // Create super circut param
    load_or_create_params(params_path.as_str(), *DEGREE).expect("failed to load or create params");
    // Create aggregator circut param
    load_or_create_params(params_path.as_str(), *AGG_DEGREE)
        .expect("failed to load or create agg-kzg params");
    // Create seed
    load_or_create_seed(seed_path.as_str()).expect("failed to load or create seed");
}
