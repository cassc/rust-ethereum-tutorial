use clap::Parser;
use ethers::contract::Contract;
use ethers::prelude::{
    Address, BlockId, BlockNumber, LocalWallet, SignerMiddleware, TransactionRequest, H160, U256,
};
use ethers::types::transaction::eip2718::TypedTransaction;
use ethers::utils::Ganache;
use ethers_providers::{Middleware, Provider};
use eyre::{ContextCompat, Result};
use hex::ToHex;
use sha3::{Digest, Keccak256};
use std::str::FromStr;
use std::time::Duration;

pub type SignerDeployedContract<T> = Contract<SignerMiddleware<Provider<T>, LocalWallet>>;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Ganache argument: Fork anther blockchain
    #[clap(short, long)]
    fork: String,
    /// Ganache argument: Unlock an address
    #[clap(short, long)]
    unlock: String,
}

/// Calculate prefix with function signature.
///
/// Input should not contain the function signature without parameter
/// names or any whitespaces. Type of the parameters should be
/// explicit, for example, use `uint256` instead of `uint`.
///
/// Returns a 8 character hex string representing a 4 byte array
pub fn fn_sig_to_prefix(fn_sig: &str) -> String {
    let ret = Keccak256::digest(fn_sig.as_bytes());
    let ret: String = ret.encode_hex();
    ret[..8].to_owned()
}

fn sep() {
    println!("{}", "=".repeat(80));
}

/// Convert wei to readable string
fn format_wei(bignumber: U256, decimals: u32, unit: Option<&str>) -> String {
    format!(
        "{:.3} {}",
        bignumber.as_u128() as f64 / u128::pow(10, decimals) as f64,
        unit.unwrap_or("")
    )
}

#[tokio::main]
async fn main() -> Result<()> {
    // Load command line arguments
    let args = Args::parse();

    let mut ganache_args: Vec<String> = Vec::new();
    let fork = args.fork;
    ganache_args.push("-f".into());
    ganache_args.push(fork.into());
    let latest_block = BlockId::Number(BlockNumber::Latest);

    let unlock = args.unlock;
    ganache_args.push("-u".into());
    ganache_args.push(unlock.clone().into());
    let unlocked_address = unlock.clone().parse::<Address>()?;

    let ganache = Ganache::new().args(ganache_args).spawn();

    sep();
    println!("HTTP Endpoint: {}", ganache.endpoint());

    // A provider is an Ethereum JsonRPC client
    let provider = Provider::try_from(ganache.endpoint())?.interval(Duration::from_millis(10));
    let chain_id = provider.get_chainid().await?.as_u64();
    println!("Ganache started with chain_id {chain_id}");

    let balance = provider.get_balance(unlocked_address, None).await?;

    println!(
        "Unlocked address {} balance: {}",
        unlock,
        format_wei(balance, 18, Some("ETH"))
    );

    // the Uniswap ERC20 contract address
    let contract = H160::from_str("0xdac17f958d2ee523a2206206994597c13d831ec7")?;
    let client = provider.clone().with_sender(unlocked_address);

    sep();
    // Constract a call to get the token symbol
    let fn_sig_hex = fn_sig_to_prefix("symbol()");
    let data = hex::decode(fn_sig_hex)?;
    let tx = TransactionRequest::new()
        .chain_id(chain_id)
        .data(data)
        .to(contract);
    let tx = TypedTransaction::Legacy(tx);
    let resp = client.call(&tx, None).await?.to_vec();
    // Drop 64 bytes and trim the rest zeros
    let resp: Vec<_> = resp.into_iter().skip(64).take_while(|c| c.gt(&0)).collect();
    let symbol = String::from_utf8_lossy(&resp);
    let token_symbol = symbol.trim();

    println!("Token Symbol: {}", token_symbol);

    // Get block number and next block gas price
    sep();
    let block = client
        .get_block(latest_block)
        .await?
        .context("Expecting to get latest block")?;

    let block_number = block.number.context("Missing block number in block")?;
    println!("Current block number {}", block_number);

    let predicted_gas_price = block
        .next_block_base_fee()
        .context("Failed to get base_fee_per_gas")?;

    // Constract a call to get the balance
    sep();
    let fn_sig_hex = fn_sig_to_prefix("balanceOf(address)");
    let fn_args_hex = format!("{:064x}", U256::from_str_radix(&unlock, 16)?);
    let data = hex::decode(format!("{}{}", fn_sig_hex, fn_args_hex))?;

    // Create a read-only contract call by getting the balance of an address:
    // To get UNI token balance for 0x1a9c8182c09f50c8318d769245bea52c32be35bc
    sep();
    let tx = TransactionRequest::new()
        .chain_id(chain_id)
        .data(data)
        .to(contract);

    let tx = TypedTransaction::Legacy(tx);
    let resp = client.call(&tx, None).await?;
    let balance = U256::from_big_endian(resp.as_ref());
    println!("Token balance: {}", format_wei(balance, 6, Some(&symbol)));

    // Create a transaction to transfer some token from
    // 0x1a9c8182c09f50c8318d769245bea52c32be35bc to some arbitrary
    // address
    sep();
    let recipient = H160::from_str("0x192F53Ba0f8aBa9F0E7Af809916d6ffE2b6A9C31")?;
    let fn_sig_hex = fn_sig_to_prefix("transfer(address,uint256)");
    let amount = U256::from_dec_str("845044608000000")?;
    let fn_args_hex = format!("{:0>64}{:064x}", recipient.encode_hex::<String>(), amount);
    println!("fn_sig_hex {}", fn_sig_hex);
    println!("fn_args_hex {}", fn_args_hex);
    let data = hex::decode(format!("{}{}", fn_sig_hex, fn_args_hex))?;

    let tx = TransactionRequest::new()
        .chain_id(chain_id)
        .data(data)
        // 1. different from read-only call, we need a sender for the transaction
        .from(unlocked_address)
        // 2. gas price is optional, however your transaction may get rejected if the default hard coded gas price is too low
        .gas_price(predicted_gas_price)
        .to(contract);

    let resp = client.send_transaction(tx, None).await?;
    // 3. Unlike read-only call, transactions need to be `mined` and we can only get a receipt of the transaction instead of the return value of the call
    let resp = resp.confirmations(1).await?.context("Missing response")?;
    println!("resp: {:?}", resp);

    let fn_sig_hex = fn_sig_to_prefix("balanceOf(address)");
    let fn_args_hex = format!("{:0>64}", recipient.encode_hex::<String>());
    let data = hex::decode(format!("{}{}", fn_sig_hex, fn_args_hex))?;
    let tx = TransactionRequest::new()
        .chain_id(chain_id)
        .data(data)
        .to(contract);

    let tx = TypedTransaction::Legacy(tx);
    let resp = client.call(&tx, None).await?;
    let balance = U256::from_big_endian(resp.as_ref());
    println!(
        "Recipient token balance: {}",
        format_wei(balance, 6, Some(&symbol))
    );

    Ok(())
}
