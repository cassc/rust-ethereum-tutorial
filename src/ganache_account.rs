use ethers::abi::Tokenize;
use ethers::contract::Contract;
use ethers::providers::{Http, Ws};
use ethers::{
    prelude::*,
    utils::{Ganache, GanacheInstance},
};
use eyre::{ContextCompat, Result};

pub type SignerDeployedContract<T> = Contract<SignerMiddleware<Provider<T>, LocalWallet>>;

pub struct GanacheAccount<T> {
    pub ganache: GanacheInstance,
    pub chain_id: u64,
    pub provider: Provider<T>,
}

#[allow(dead_code)]
impl GanacheAccount<Http> {
    pub async fn new_from_seed(seed_words: String) -> Result<Self> {
        let ganache = Ganache::new().mnemonic(seed_words).spawn();
        let provider = Provider::try_from(ganache.endpoint())?;
        let chain_id = provider.get_chainid().await?.as_u64();
        Ok(GanacheAccount {
            ganache,
            chain_id,
            provider,
        })
    }

    pub async fn new_from_seed_with_args(seed_words: String, args: Vec<String>) -> Result<Self> {
        let ganache = Ganache::new().args(args).mnemonic(seed_words).spawn();
        let provider = Provider::try_from(ganache.endpoint())?;
        let chain_id = provider.get_chainid().await?.as_u64();
        Ok(GanacheAccount {
            ganache,
            chain_id,
            provider,
        })
    }
}

#[allow(dead_code)]
impl GanacheAccount<Ws> {
    pub async fn new_from_seed(seed_words: String) -> Result<Self> {
        let ganache = Ganache::new().mnemonic(seed_words).spawn();
        let provider = Provider::connect(ganache.ws_endpoint()).await?;
        let chain_id = provider.get_chainid().await?.as_u64();
        Ok(GanacheAccount {
            ganache,
            chain_id,
            provider,
        })
    }
}

#[allow(dead_code)]
impl<T: 'static + Clone + JsonRpcClient> GanacheAccount<T> {
    pub fn get_default_wallet(&self) -> Result<LocalWallet> {
        self.get_wallet(0)
    }

    pub fn get_chain_id(&self) -> u64 {
        self.chain_id
    }

    pub fn get_wallet(&self, i: usize) -> Result<LocalWallet> {
        let wallet = self
            .ganache
            .keys()
            .get(i)
            .context("Wallet not found at this index")?
            .clone()
            .into();
        Ok(wallet)
    }

    pub async fn deploy_contract<A: Tokenize>(
        &self,
        contract: ConfigurableContractArtifact,
        constructor_args: A,
    ) -> Result<SignerDeployedContract<T>> {
        let (abi, bytecode, _) = contract.into_parts();
        let abi = abi.context("Missing abi from contract")?;
        let bytecode = bytecode.context("Missing bytecode from contract")?;

        // Create signer client
        let wallet = self.get_default_wallet()?.with_chain_id(self.chain_id);
        let client = SignerMiddleware::new(self.provider.clone(), wallet).into();

        // Deploy contract
        let factory = ContractFactory::new(abi.clone(), bytecode, client);
        let mut deployer = factory.deploy(constructor_args)?;
        let block = self
            .provider
            .clone()
            .get_block(BlockNumber::Latest)
            .await?
            .context("Failed to get latest block")?;

        let gas_price = block
            .next_block_base_fee()
            .context("Failed to get the next block base fee")?;
        // let gas_limit = block.gas_limit;

        // We can also manually set the gas limit
        // deployer.tx.set_gas::<U256>(gas_limit);
        deployer.tx.set_gas_price::<U256>(gas_price);
        let contract = deployer.clone().legacy().send().await?;
        Ok(contract)
    }
}
