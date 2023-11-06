use std::{fs::File, io::Write};

use clap::Parser;
use halo2_proofs::{halo2curves::bn256::Bn256, poly::{kzg::commitment::ParamsKZG, commitment::Params}};
use zkevm_prover::utils::{FS_PROVE_PARAMS, FS_PROVE_SEED};

const RNG: XorShiftRng = XorShiftRng::from_seed([
    0x59, 0x62, 0xbe, 0x5d, 0x76, 0x3d, 0x31, 0x8d, 0x17, 0xdb, 0x37, 0x32, 0x54, 0x06, 0xbc, 0xe5,
]);

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
    load_or_create_params(params_path.as_str(), 16);
    // Create aggregator circut param
    load_or_create_params(params_path.as_str(), 25);
    // Create seed
    // load_or_create_seed(seed_path.as_str()).expect("failed to load or create seed");
}

fn load_or_create_params(params_path: &str, degree: u32) {
    let params = ParamsKZG::<Bn256>::unsafe_setup_with_s(degree, RNG);
    let mut params_buf = Vec::new();
    params.write(&mut params_buf).unwrap();

    let mut params_file = File::create(&params_path).unwrap();
    params_file.write_all(&params_buf[..]).unwrap();
    log::info!("create params successfully!");
}
