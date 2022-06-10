use ethers::contract::Contract;
use ethers::prelude::{
    BlockNumber, ConfigurableArtifacts, ContractFactory, LocalWallet, Project,
    ProjectCompileOutput, ProjectPathsConfig, Signer, SignerMiddleware, U256,
};
use ethers::utils::Ganache;
use ethers_providers::{Middleware, Provider};
use ethers_solc::Artifact;
use eyre::Result;
use eyre::{eyre, ContextCompat};
use hex::ToHex;
use std::path::PathBuf;
use std::time::Duration;

pub type SignerDeployedContract<T> = Contract<SignerMiddleware<Provider<T>, LocalWallet>>;

#[tokio::main]
async fn main() -> Result<()> {
    // Spawn a ganache instance
    let mnemonic = "gas monster ski craft below illegal discover limit dog bundle bus artefact";
    let ganache = Ganache::new().mnemonic(mnemonic).spawn();
    println!("HTTP Endpoint: {}", ganache.endpoint());

    // Get the first wallet managed by ganache
    let wallet: LocalWallet = ganache.keys()[0].clone().into();
    let first_address = wallet.address();
    println!(
        "Wallet first address: {}",
        first_address.encode_hex::<String>()
    );

    // A provider is an Ethereum JsonRPC client
    let provider = Provider::try_from(ganache.endpoint())?.interval(Duration::from_millis(10));
    let chain_id = provider.get_chainid().await?.as_u64();
    println!("Ganache started with chain_id {chain_id}");

    // Compile solidity project
    let project = compile("examples/").await?;

    // Print compiled project information
    print_project(project.clone()).await?;

    let balance = provider.get_balance(wallet.address(), None).await?;

    println!(
        "Wallet first address {} balance: {}",
        wallet.address().encode_hex::<String>(),
        balance
    );

    let contract_name = "BUSDImplementation";

    // Find the contract to be deployed
    let contract = project
        .find(contract_name)
        .context("Contract not found")?
        .clone();

    // We'll create a transaction which will include code for deploying the contract
    // Get ABI and contract byte, these are required for contract deployment
    let (abi, bytecode, _) = contract.into_parts();
    let abi = abi.context("Missing abi from contract")?;
    let bytecode = bytecode.context("Missing bytecode from contract")?;

    // Create signer client
    let wallet = wallet.with_chain_id(chain_id);
    let client = SignerMiddleware::new(provider.clone(), wallet).into();

    // Deploy contract
    let factory = ContractFactory::new(abi.clone(), bytecode, client);
    // Our contract don't need any constructor arguments, so we can use an empty tuple
    let mut deployer = factory.deploy(())?;
    let block = provider
        .clone()
        .get_block(BlockNumber::Latest)
        .await?
        .context("Failed to get latest block")?;

    // Set a reasonable gas price to prevent our contract from being rejected by EVM
    let gas_price = block
        .next_block_base_fee()
        .context("Failed to get the next block base fee")?;
    deployer.tx.set_gas_price::<U256>(gas_price);

    // We can also manually set the gas limit
    // let gas_limit = block.gas_limit;
    // deployer.tx.set_gas::<U256>(gas_limit);

    // Create transaction and send
    let contract = deployer.clone().legacy().send().await?;

    println!(
        "BUSDImpl contract address {}",
        contract.address().encode_hex::<String>()
    );

    Ok(())
}

pub async fn compile(root: &str) -> Result<ProjectCompileOutput<ConfigurableArtifacts>> {
    // Create path from string and check if the path exists
    let root = PathBuf::from(root);
    if !root.exists() {
        return Err(eyre!("Project root {root:?} does not exists!"));
    }

    // Configure `root` as our project root
    let paths = ProjectPathsConfig::builder()
        .root(&root)
        .sources(&root)
        .build()?;

    // Create a solc ProjectBuilder instance for compilation
    let project = Project::builder()
        .paths(paths)
        .set_auto_detect(true) // auto detect solc version from solidity source code
        .no_artifacts()
        .build()?;

    // Compile project
    let output = project.compile()?;

    // Check for compilation errors
    if output.has_compiler_errors() {
        Err(eyre!(
            "Compiling solidity project failed: {:?}",
            output.output().errors
        ))
    } else {
        Ok(output.clone())
    }
}

pub async fn print_project(project: ProjectCompileOutput<ConfigurableArtifacts>) -> Result<()> {
    let artifacts = project.into_artifacts();
    for (id, artifact) in artifacts {
        let name = id.name;
        let abi = artifact.abi.context("No ABI found for artificat {name}")?;

        println!("{}", "=".repeat(80));
        println!("CONTRACT: {:?}", name);

        let contract = &abi.abi;
        let functions = contract.functions();
        let functions = functions.cloned();
        let constructor = contract.constructor();

        if let Some(constructor) = constructor {
            let args = &constructor.inputs;
            println!("CONSTRUCTOR args: {args:?}");
        }

        for func in functions {
            let name = &func.name;
            let params = &func.inputs;
            println!("FUNCTION  {name} {params:?}");
        }
    }
    Ok(())
}
