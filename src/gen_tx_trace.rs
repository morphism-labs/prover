extern crate types as prove_types;
use ethers::prelude::*;
use ethers::providers::{Http, Provider};
use ethers::signers::Wallet;
use ethers::types::Address;
use prove_types::eth::BlockTrace;
use std::{error::Error, str::FromStr, sync::Arc};

const CONTRACT_ADDRESS: &str = "0x";
const PRIVATE_KEY: &str = "0x";

// Generate tx for traces.
#[tokio::test]
async fn test() -> Result<(), Box<dyn Error>> {
    let result = call().await;
    match result {
        Ok(()) => Ok(()),
        Err(e) => {
            println!("call error:");
            Err(e)
        }
    }
}

// Call a contract.
async fn call() -> Result<(), Box<dyn Error>> {
    let provider: Provider<Http> = Provider::<Http>::try_from("http://127.0.0.1:8569")?;
    let wallet: LocalWallet = Wallet::from_str(PRIVATE_KEY)?;

    let signer = Arc::new(SignerMiddleware::new(
        provider.clone(),
        wallet.with_chain_id(1337 as u64),
    ));

    abigen!(TestZkEVM, "./resource/abi/TestZkEVM.json");

    let contract: TestZkEVM<SignerMiddleware<Provider<Http>, _>> =
        TestZkEVM::new(Address::from_str(CONTRACT_ADDRESS)?, signer);

    let tx = contract
        .transfer(Address::from_str("0x").unwrap(), 10.into())
        .legacy();
    let receipt = tx.send().await;
    match receipt {
        Ok(sent_tx) => println!("====transaction ID: {:?}", sent_tx),
        Err(e) => println!("call exception: {:?}", e),
    }
    Ok(())
}

// Deploy a contract.
async fn deploy() -> Result<(), Box<dyn Error>> {
    let provider: Provider<Http> = Provider::<Http>::try_from("http://127.0.0.1:8569")?;
    let wallet: LocalWallet = Wallet::from_str(PRIVATE_KEY)?;

    let signer = Arc::new(SignerMiddleware::new(
        provider.clone(),
        wallet.with_chain_id(1337 as u64),
    ));
    // let factory = ContractFactory::new(abi, bytecode, client.clone());

    abigen!(TestZkEVM, "./resource/abi/TestZkEVM.json");
    let a: u64 = 10;

    let tx = TestZkEVM::deploy(signer, a.pow(18))?.legacy();
    let contract = tx.send().await;

    match contract {
        Ok(sent_tx) => println!("====testZkEVM: {:?}", sent_tx),
        Err(e) => println!("deploy exception: {:?}", e),
    }

    Ok(())
}
