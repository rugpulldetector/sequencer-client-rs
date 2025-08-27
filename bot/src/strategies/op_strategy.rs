use alloy_primitives::TxKind;
use grouping_by::GroupingBy;
use revm_trace::{create_shared_backend, evm::builder::get_provider, revm::{context::result::ExecutionResult, database::states::changes}, types::StateOverride, SharedBackend, SimulationBatch, SimulationTx, TransactionTrace};
use tokio::{io::Join, sync::mpsc::{UnboundedReceiver, UnboundedSender}, task::JoinSet};
use ethers::{core::k256::elliptic_curve::consts::U25, middleware::gas_oracle::cache, types::{transaction::eip2718::TypedTransaction, Eip1559TransactionRequest, H160, H256, I256, U256, Bytes}, utils::{format_ether, keccak256, WEI_IN_ETHER}};
use ethers_providers::{spoof, Http, Provider, RawCall};
use tokio::sync::RwLock;
use std::{collections::HashMap, str::FromStr, sync::Arc};
use ethers::abi::AbiDecode;

use crate::{abi::{MSLauncher, IERC20}, collectors::{binance_collector::start_binance_collector, block_collector::{start_block_collector, BlockInfo}, flash_block_collector::start_flash_block_collector}, types::{calculate_balance_slot, decode_price, find_best_trade_list, simulate_tx_with_revm, sqrt_price_x96_to_price, to_spoof_state, to_state_override, FlashblocksPayloadV1, LogReceipts, TradeInfo}};

#[derive(Clone)]
struct LauncherInfo {
    pub launcher_name: String,
    pub launcher_addr: H160,
    pub simulator_addr: H160,
    pub base_token_addr: H160,
    pub zero_for_one: bool,
    pub decimal0: u8,
    pub decimal1: u8,
    pub min_profit: U256,
    pub min_swap_amount: U256,
}


pub struct OpStrategy {
    pub from_addr: H160,
    pub launcher_info_list: Vec<LauncherInfo>,
    pub rpc_url: String,
    pub rpc_client  : Arc<Provider<Http>>,
    pub chain_id: u64,  
    pub base_balance_map: HashMap<H160, Vec<U256>>,

    pub block_info: BlockInfo,
    pub block_info_reader: Arc<RwLock<BlockInfo>>,

    pub eth_price: Arc<RwLock<U256>>,
    pub op_price: Arc<RwLock<U256>>,
    pub binance_price_reader: Arc<RwLock<HashMap<String, U256>>>,

    pub trade_info_map: Arc<RwLock<HashMap<(H160, usize, bool), Vec<TradeInfo>>>>,

    // pub block_ws_url: String,
    pub tx_sender: UnboundedSender<TypedTransaction>,

    pub gas_limit: u64,

    pub test_mode: bool,
}

impl OpStrategy {
    pub async fn new(
        rpc_url: String,
        rpc_client: Arc<Provider<Http>>,
        from_addr: H160,
        chain_id: u64,
        block_ws_url: String,
        tx_sender: UnboundedSender<TypedTransaction>,
        gas_limit: u64,
        test_mode: bool,
    ) -> Self {
        let from_addr = from_addr;
        
        // let launcher_info_list = vec![LauncherInfo {
        //     launcher_name: "WETH_OP".to_string(),
        //     launcher_addr: H160::from_str("0xa63f733EBF5E5E5fB360809209Fe62CF5d256C5f").unwrap(),
        //     base_token_addr: H160::from_str("0x4200000000000000000000000000000000000006").unwrap(),
        // },    
        // LauncherInfo {
        //     launcher_name: "WETH_USDC".to_string(),
        //     launcher_addr: H160::from_str("0x0477Fa1eC254b79cF78660589C1d701e43B83F70").unwrap(),
        //     base_token_addr: H160::from_str("0x4200000000000000000000000000000000000006").unwrap(),
        // },
        // LauncherInfo {
        //     launcher_name: "WETH_USDC_E".to_string(),
        //     launcher_addr: H160::from_str("0x94CeCc72F28453E5E317278eA15AB2f623d8C3F1").unwrap(),
        //     base_token_addr: H160::from_str("0x4200000000000000000000000000000000000006").unwrap(),
        // },
        // LauncherInfo {
        //     launcher_name: "OP_USDC".to_string(),
        //     launcher_addr: H160::from_str("0x9036Dd0d39A5A87B85f4D8950F07824C576c5853").unwrap(),
        //     base_token_addr: H160::from_str("0x4200000000000000000000000000000000000042").unwrap(),
        // },
        // LauncherInfo {
        //     launcher_name: "OP_USDC_E".to_string(),
        //     launcher_addr: H160::from_str("0xd241F0D7260B3eb44F67f36B2E7AD72eAe5aE8aA").unwrap(),
        //     base_token_addr: H160::from_str("0x4200000000000000000000000000000000000042").unwrap(),
        // }
        // ];

        let launcher_info_list = vec![
            LauncherInfo {
                launcher_name: "WETH_USDC".to_string(),
                launcher_addr: H160::from_str("0xBa9e959f472eE197Ac1518a99E7435dF0ECefd30").unwrap(),
                simulator_addr: H160::from_str("0xdea14e1cE824878F3f71a72Ee0F415B9bddB0F17").unwrap(),
                base_token_addr: H160::from_str("0x4200000000000000000000000000000000000006").unwrap(),
                zero_for_one: false,
                decimal0: 18,
                decimal1: 6,
                min_profit: WEI_IN_ETHER / 10000,
                min_swap_amount: WEI_IN_ETHER * 3 / 2,
            },
            LauncherInfo {
                launcher_name: "WETH_OP".to_string(),
                launcher_addr: H160::from_str("0xA011FE071308218c6A064f1fDeaC3Db82Ee5f540").unwrap(),
                simulator_addr: H160::from_str("0xEB30899D937e0825e00744eAec82fC966a189205").unwrap(),
                base_token_addr: H160::from_str("0x4200000000000000000000000000000000000006").unwrap(),
                zero_for_one: true,
                decimal0: 18,
                decimal1: 18,
                min_profit: WEI_IN_ETHER / 10000,
                min_swap_amount: WEI_IN_ETHER * 3 / 2,
            }, 
            LauncherInfo {
                launcher_name: "OP_USDC".to_string(),
                launcher_addr: H160::from_str("0x29aD65432121a9B5C1fE3afaa73992295A7724B8").unwrap(),
                simulator_addr: H160::from_str("0xc2843c182ed351bd533e32485a3e24dA67df0ce2").unwrap(),
                base_token_addr: H160::from_str("0x4200000000000000000000000000000000000042").unwrap(),
                zero_for_one: false,
                decimal0: 18,
                decimal1: 6,
                min_profit: WEI_IN_ETHER / 10,
                min_swap_amount: WEI_IN_ETHER * 3 / 2,
            },
        ];
        
        let base_balance_map = Self::get_base_balance_list(rpc_client.clone(), launcher_info_list.clone()).await;

        let binance_price_reader = start_binance_collector(vec!["ETHUSDT".to_string(), "OPUSDT".to_string()]).await;
        
        let eth_price = Arc::new(RwLock::new(U256::from(0)));
        let op_price = Arc::new(RwLock::new(U256::from(0)));
        for (symbol, price) in binance_price_reader.read().await.clone() {
            if symbol == "ETHUSDT" {
                *eth_price.write().await = price;
            } else if symbol == "OPUSDT" {
                *op_price.write().await = price;
            }
        }

        let block_info_reader = start_block_collector(block_ws_url).await;
        let block_info = block_info_reader.read().await.clone();

        let trade_info_map = Arc::new(RwLock::new(HashMap::new()));

        Self {
            rpc_url,
            rpc_client,
            from_addr,
            launcher_info_list,
            chain_id,
            block_info,
            block_info_reader,
            eth_price,
            op_price,
            binance_price_reader,
            base_balance_map,
            trade_info_map,                    
            tx_sender,
            gas_limit,
            test_mode,
        }
    }

    pub async fn run(&mut self) {
        loop {
            let block_info = self.block_info_reader.read().await.clone();

            if block_info.number != self.block_info.number {
                println!("Block: {}", block_info.number);
                self.block_info = block_info;

                // Update trade info list
                if block_info.number % 10 == 0.into() {
                    self.update_trade_info_map().await;
                }

                // Update base balance map
                if self.block_info.number % 100 == 0.into() {            
                    self.base_balance_map = Self::get_base_balance_list(self.rpc_client.clone(), self.launcher_info_list.clone()).await;
                }

                // check if there is any trade with deviation > 60 bps
                self.check_trade_opportunity().await;
            }

            // update binance price
            let binance_price = self.binance_price_reader.read().await.clone();
            for (symbol, price) in binance_price.iter() {
                if symbol == "ETHUSDT" {
                    *self.eth_price.write().await = *price;
                } else if symbol == "OPUSDT" {
                    *self.op_price.write().await = *price;
                }
            }
            
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }

    async fn get_base_balance_list(rpc_client: Arc<Provider<Http>>, launcher_info_list: Vec<LauncherInfo>) -> HashMap<H160, Vec<U256>> {
        let mut base_balance_map = HashMap::new();
        for launcher_info in launcher_info_list.clone() {
            let launcher = MSLauncher::new(launcher_info.launcher_addr, rpc_client.clone());
            base_balance_map.insert(launcher_info.launcher_addr, launcher.get_base_balance_list().call().await.unwrap());
        }
        base_balance_map
    }

    async fn update_trade_info_map(&self) {
        let launcher_info_list = self.launcher_info_list.clone();
        let rpc_client = self.rpc_client.clone();
        let rpc_url = self.rpc_url.clone();
        let block_info = self.block_info.clone();
        let from_addr = self.from_addr;
        let chain_id = self.chain_id;
        let base_balance_map = self.base_balance_map.clone();
        let trade_info_map = self.trade_info_map.clone();

        tokio::spawn(async move {
            let trade_info_map_new: HashMap<(H160, usize, bool), Vec<TradeInfo>> =
                simulate_trade(
                    launcher_info_list.clone(),
                    rpc_client,
                    rpc_url.as_str(),
                    block_info,
                    from_addr,
                    chain_id,
                    base_balance_map).await;         

            *trade_info_map.write().await = trade_info_map_new;
        });
    }

    async fn check_trade_opportunity(&self) {
        let launcher_info_list = self.launcher_info_list.clone();
        let eth_price = *self.eth_price.read().await;
        let op_price = *self.op_price.read().await;
        let eth_op_price = if op_price == U256::from(0) {
            U256::from(0)
        } else {
            eth_price * 100000000 / op_price
        };

        let trade_info_list = self.trade_info_map.read().await;
    
        for ((launcher_addr, pool_index, sell_base_token), trade_info_list) in trade_info_list.iter() {
            let mut ops = vec![];
            let launcher_info = launcher_info_list.iter().find(|launcher_info| launcher_info.launcher_addr == *launcher_addr).unwrap();
            let binance_price = 
                if launcher_info.launcher_name == "WETH_USDC" {
                    eth_price
                } else if launcher_info.launcher_name == "WETH_OP" {
                    eth_op_price
                } else {
                    op_price
                };

            if binance_price == U256::from(0) {
                continue;
            }

            for mut op in trade_info_list.clone() {
                op.trade_price = sqrt_price_x96_to_price(op.sqrt_price_x96.clone(),
                    launcher_info.zero_for_one,
                    launcher_info.decimal0,
                    launcher_info.decimal1);
    
                op.deviation_bps = if binance_price > op.trade_price {
                        binance_price - op.trade_price
                    } else {
                        op.trade_price - binance_price
                    } * 10000 / binance_price;

                if op.deviation_bps < U256::from(50) {
                    ops.push(op);
                }
            }

            if ops.is_empty() {
                continue;
            }

            println!("{} - Index: {} - {} - Binance: {}",
                launcher_info.launcher_name,
                *pool_index,
                if *sell_base_token { "Bid" } else { "Ask" },                    
                binance_price);

            for op in ops.iter() {  
                println!("Pool: {} - Deviation(bps): {} - Delta: {} - Swap Amount: {} - Profit: {} - Gas: {}",
                op.trade_price,
                op.deviation_bps,
                format_ether(op.start_delta.clone()),
                format_ether(op.swap_amount.clone()),
                format_ether(op.profit.clone()),
                op.gas_used);    
            }
        }
    
    }
}

async fn simulate_trade(
    launcher_info_list: Vec<LauncherInfo>,
    rpc_client: Arc<Provider<Http>>,
    rpc_url: &str,
    block_info: BlockInfo,
    from_addr: H160,
    chain_id: u64,
    base_balance_map: HashMap<H160, Vec<U256>>,
) -> HashMap<(H160, usize, bool), Vec<TradeInfo>>
{
    let shared_backend = create_shared_backend(
        rpc_url.clone(),
        Some(block_info.number.as_u64().into())).await.unwrap();
    let provider = Arc::new(get_provider(rpc_url.clone()).await.unwrap());

    let mut join_set = JoinSet::new();

    for launcher_info in launcher_info_list.iter() {
        let launcher_addr = launcher_info.launcher_addr;
        let simulator_addr = launcher_info.simulator_addr;
        let weth_addr = H160::from_str("0x4200000000000000000000000000000000000006").unwrap();
        let weth_balance_slot = calculate_balance_slot(simulator_addr, 3);
        let weth_balance_value = H256::from_str("0x00000000000000000000000000000000000000000000d3c21bcecceda1000000").unwrap();
        let op_addr = H160::from_str("0x4200000000000000000000000000000000000042").unwrap();
        let op_balance_slot = calculate_balance_slot(simulator_addr, 0);
        let op_balance_value = H256::from_str("0x000000000000000000000000000000000000000000ddd3c21bcecceda1000000").unwrap();
        let usdc_addr = H160::from_str("0x0b2C639c533813f4Aa9D7837CAf62653d097Ff85").unwrap();
        let usdc_balance_slot = calculate_balance_slot(simulator_addr, 9);
        let usdc_balance_value = H256::from_str("0x0000000000000000000000000000000000000000000000000de0b6b3a7640000").unwrap();                
        let usdc_e_addr = H160::from_str("0x7F5c764cBc14f9669B88837ca1490cCa17c31607").unwrap();
        let usdc_e_balance_slot = calculate_balance_slot(simulator_addr, 0);
        let usdc_e_balance_value = H256::from_str("0x0000000000000000000000000000000000000000000000000de0b6b3a7640000").unwrap();                
        let base_balance_list = base_balance_map.get(&launcher_addr).unwrap();
        let pool_count = base_balance_list.len();
        let min_profit = launcher_info.min_profit;
        let min_swap_amount = launcher_info.min_swap_amount;

        let storage_changes = vec![
            (weth_addr, weth_balance_slot, weth_balance_value),
            (op_addr, op_balance_slot, op_balance_value),
            (usdc_addr, usdc_balance_slot, usdc_balance_value),
            (usdc_e_addr, usdc_e_balance_slot, usdc_e_balance_value)];

        // Clone values that need to be moved into spawned tasks
        let storage_changes_clone = storage_changes.clone();
        let base_balance_list_clone = base_balance_list.clone();

        // let spoof_state = to_spoof_state(&storage_changes);

        for sell_base_token in [true, false] {                    
            for pool_index in 0..pool_count {
                let shared_backend = shared_backend.clone();
                let rpc_client = rpc_client.clone();
                let provider = provider.clone();
                let storage_changes = storage_changes_clone.clone();
                let starting_delta = base_balance_list_clone[pool_index] / 3;
                
                join_set.spawn(async move {
                    find_best_trade_list(
                        shared_backend,
                        provider,
                        rpc_client,
                        from_addr,
                        launcher_addr,
                        simulator_addr,
                        min_profit,
                        min_swap_amount,
                        storage_changes,
                        pool_index,
                        sell_base_token,
                        starting_delta).await
                });
            }
        }
    }

    let trade_info_list: Vec<TradeInfo> = join_set.join_all().await.into_iter().flatten().collect();

    trade_info_list.into_iter().grouping_by(|trade_info| 
        (trade_info.launcher_addr, trade_info.pool_index, trade_info.sell_base_token))
}
