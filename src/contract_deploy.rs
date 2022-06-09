use std::path::PathBuf;
mod ganache_account;
use clap::Parser;
use ethers_providers::{Http, Middleware};
use eyre::{eyre, ContextCompat};
use ganache_account::GanacheAccount;
use hex::ToHex;

use ethers::prelude::{
    Address, ConfigurableArtifacts, Project, ProjectCompileOutput, ProjectPathsConfig, Signer,
    TransactionRequest, U256,
};
use eyre::Result;
use tracing::{instrument, Level};

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Directory holding our contracts
    #[clap(short, long, required = true)]
    project_root: String,
    /// Set to 1 to enable tracing
    #[clap(short, long)]
    tracing: bool,
    /// Ganache argument: Fork anther blockchain
    #[clap(short, long)]
    fork: Option<String>,
    /// Ganache argument: Unlock an address
    #[clap(short, long)]
    unlock: Option<String>,
    /// Ganache argument: gas price
    #[clap(short, long)]
    gas_price: Option<u32>,
}

fn enable_tracing() -> Result<()> {
    let collector = tracing_subscriber::fmt()
        .with_max_level(Level::TRACE)
        .finish();

    tracing::subscriber::set_global_default(collector)?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Load command line arguments
    let args = Args::parse();
    let root: String = args.project_root;

    // Enable printing tracing for EVM ethereum libraries
    if args.tracing {
        enable_tracing()?;
    }

    // Compile project
    let project = compile(&root).await?;

    // Print compiled project information
    // print_project(project.clone()).await?;

    // Create a ganache instance using some seed phrases
    let seed = "kiss flower cover latin day egg tree fabric acoustic cheap energy usual".into();
    let mut ganache_args: Vec<String> = Vec::new();
    if let Some(fork) = args.fork {
        ganache_args.push("-f".into());
        ganache_args.push(fork.into());
    }

    if let Some(gas_price) = args.gas_price {
        ganache_args.push("-g".into());
        ganache_args.push(gas_price.to_string());
    }

    let mut unlocked_address = None;

    if let Some(ref unlock) = args.unlock {
        ganache_args.push("-u".into());
        ganache_args.push(unlock.into());
        unlocked_address = Some(unlock.parse::<Address>()?);
    }

    let ganache_account = {
        if ganache_args.is_empty() {
            GanacheAccount::<Http>::new_from_seed(seed).await?
        } else {
            GanacheAccount::<Http>::new_from_seed_with_args(seed, ganache_args).await?
        }
    };

    // Get default wallet in the account and query balance
    let wallet = ganache_account.get_default_wallet()?;

    let balance = ganache_account
        .provider
        .get_balance(wallet.address(), None)
        .await?;

    println!(
        "Wallet first address {} balance: {}",
        wallet.address().encode_hex::<String>(),
        balance
    );

    let token_impl_contract_name = "BUSDImplementation";
    let proxy_contract_name = "AdminUpgradeabilityProxy";

    // Find and deploy the token implementation contract
    let contract = project
        .find(token_impl_contract_name)
        .context("Contract not found")?
        .clone();
    let token_impl_contract = ganache_account.deploy_contract(contract, ()).await?;

    println!(
        "BUSDImpl contract address {}",
        token_impl_contract.address().encode_hex::<String>()
    );

    // Find and deploy the proxy contract
    let contract = project
        .find(proxy_contract_name)
        .context("Contract not found")?
        .clone();

    // let constructor = contract.into_parts().0.context("Failed to get contract parts")?.constructor?;
    let constructor_args = (token_impl_contract.address(),);
    let proxy_contract = ganache_account
        .deploy_contract(contract, constructor_args)
        .await?;

    println!(
        "BUSD contract address {}",
        proxy_contract.address().encode_hex::<String>()
    );

    if let Some(ref unlocked_address) = unlocked_address {
        let gas_price = U256::from(40_000_000_000u128);
        let tx = TransactionRequest::pay(wallet.address(), U256::from(99999u64))
            .from(*unlocked_address)
            .gas_price(gas_price);

        let _receipt = ganache_account
            .provider
            .send_transaction(tx, None)
            .await?
            .log_msg("Pending transfer")
            .confirmations(1) // number of confirmations required
            .await?
            .context("Missing receipt")?;

        println!(
            "Balance of {} {}",
            wallet.address().encode_hex::<String>(),
            ganache_account
                .provider
                .get_balance(wallet.address(), None)
                .await?
        );
    }

    Ok(())
}

#[instrument]
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

    // Create a solc project instance for compilation
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
