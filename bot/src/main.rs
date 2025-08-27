use std::env;
use std::fs::read_to_string;
use std::str::FromStr;
use std::sync::Arc;

use ethers::signers::{LocalWallet, Signer};
use ethers::types::H160;
use ethers::utils::WEI_IN_ETHER;
use ethers_providers::{Http, Middleware};
use ethers_providers::Provider;
use ms_bot::executors::sequencer_executor::start_sequencer_executor;
use ms_bot::executors::titan_executor::start_titan_executor;
use ms_bot::strategies::base_strategy::BaseStrategy;
use ms_bot::strategies::mainnet_strategy::MainnetStrategy;
use ms_bot::strategies::op_strategy::OpStrategy;
use serde::Deserialize;
use serde::Serialize;


#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub chain_id: u64,
    pub rpc_url: String,
    pub ws_url: String,
    pub flashblocks_url: String,
    pub trade_server_url: String,
    pub mevshare_url: String,
    pub sequencer_url: String,
    pub from_addr: String,
    pub to_addr: String,
    pub simulator_addr: String,
    pub data: String,
    pub simulation_mode: bool,
    pub test_mode: bool,
    pub gas_limit: u64,
    pub step_count: u64,
    pub regression_count: u64,
    pub min_profit: u64,
    pub min_swap_amount: u64,
    pub exeuction_mode: bool,
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    let platform = &args[1];
    println!("Starting bot on {platform}");

    let cfg = read_to_string(format!("config_{platform}.toml")).unwrap();
    let cfg: Config = toml::de::from_str(&cfg).unwrap();
    let from_addr = H160::from_str(&cfg.from_addr).unwrap();
    let to_addr = H160::from_str(&cfg.to_addr).unwrap();
    let simulator_addr = H160::from_str(&cfg.simulator_addr).unwrap();

    println!("Connecting {}", cfg.rpc_url);
    let rpc_client = Arc::new(Provider::<Http>::try_from(cfg.rpc_url.clone()).unwrap());

    if platform == "base" || platform == "op" {
        let (tx_sender, tx_receiver) = tokio::sync::mpsc::unbounded_channel();

        if cfg.exeuction_mode {
            println!("Adding sequencer executor...");
            let password = rpassword::prompt_password("").unwrap();
            let wallet = LocalWallet::decrypt_keystore(cfg.data, password).unwrap();
            let wallet = wallet.with_chain_id(cfg.chain_id);
            
            let nonce = rpc_client.clone().get_transaction_count(from_addr, None).await.unwrap();
            println!("Nonce: {:?}", nonce);
            start_sequencer_executor(cfg.sequencer_url, wallet, cfg.chain_id, nonce, tx_receiver).await;
        }
            
        if platform == "base" {
            println!("Adding base strategy...");
    
            let mut strategy = BaseStrategy::new(
                cfg.rpc_url.clone(),
                cfg.ws_url.clone(),
                cfg.flashblocks_url.clone(),
                cfg.trade_server_url.clone(),
                rpc_client.clone(), 
                from_addr, 
                to_addr,
                simulator_addr,
                cfg.chain_id,
                tx_sender.clone(),
                cfg.gas_limit,
                cfg.simulation_mode,
                cfg.test_mode,
                cfg.step_count,
                cfg.regression_count,
                WEI_IN_ETHER *  cfg.min_profit / 10000,
                WEI_IN_ETHER *  cfg.min_swap_amount / 10000,
            ).await;
    
            strategy.run().await;
        }  else if platform == "op" {
            println!("Adding op strategy...");
    
            let mut strategy = OpStrategy::new(
                cfg.rpc_url.clone(),
                rpc_client.clone(),
                from_addr,
                cfg.chain_id,
                cfg.ws_url,
                tx_sender.clone(),
                cfg.gas_limit,
                cfg.test_mode
            ).await;
    
            strategy.run().await;
        }        
    } else if platform == "mainnet" {
        let (tx_sender, tx_receiver) = tokio::sync::mpsc::unbounded_channel();

        let password = rpassword::prompt_password("").unwrap();
        let wallet = LocalWallet::decrypt_keystore(cfg.data, password).unwrap();
        let wallet = wallet.with_chain_id(cfg.chain_id);
        println!("Adding titan executor...");
        start_titan_executor(rpc_client.clone(), wallet, cfg.chain_id, tx_receiver).await;

        println!("Adding mainnet strategy...");
        let mut strategy = MainnetStrategy::new(    
            rpc_client.clone(), 
            cfg.ws_url, 
            cfg.mevshare_url,
            from_addr, 
            to_addr,
            simulator_addr,
            tx_sender.clone(),
            cfg.gas_limit,
            cfg.test_mode,
            cfg.step_count,
            cfg.regression_count,
        ).await;

        strategy.run().await;
    }
}