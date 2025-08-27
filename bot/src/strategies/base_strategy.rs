use alloy_primitives::{hex::const_check_raw, TxKind};
use grouping_by::GroupingBy;
use jsonrpsee::{core::client::{ClientT, SubscriptionClientT}, rpc_params, ws_client::WsClientBuilder};
use jsonrpsee_ws_server::{RpcModule, SubscriptionSink, WsServerBuilder};
use revm_trace::{alloy::rpc::client, create_shared_backend, evm::builder::get_provider, revm::{bytecode::eof::printer::print, context::result::ExecutionResult, precompile::blake2}, types::StateOverride, SimulationBatch, SimulationTx, TransactionTrace};
use tokio::{sync::mpsc::{UnboundedReceiver, UnboundedSender}, task::JoinSet};
use ethers::{core::k256::elliptic_curve::consts::U25, middleware::gas_oracle::cache, types::{transaction::eip2718::TypedTransaction, BlockId, BlockNumber, Eip1559TransactionRequest, H160, H256, I256, U256}, utils::{format_ether, keccak256, WEI_IN_ETHER}};
use ethers_providers::{spoof, Http, Middleware, Provider, RawCall};
use tokio::sync::{Mutex, RwLock};
use tracing::subscriber;
use std::{collections::HashMap, str::FromStr, sync::Arc, time::SystemTime};
use ethers::abi::AbiDecode;

use crate::{abi::{MSLauncher, IERC20}, collectors::{binance_collector::start_binance_collector, block_collector::{start_block_collector, BlockInfo}, flash_block_collector::start_flash_block_collector}, types::{broadcast_trade, calculate_balance_slot, decode_price, find_best_trade_list, price_to_sqrt_price_x96, sqrt_price_x96_to_price, start_trade_collector, start_trade_server, FlashblocksPayloadV1, LogReceipts, PoolType, TradeInfo}};


pub struct BaseStrategy {
    pub chain_id: u64,

    pub simulation_mode: bool,

    pub from_addr: H160,
    pub to_addr: H160,
    pub simulator_addr: H160,
    pub rpc_url: String,
    pub ws_url: String,
    pub flashblocks_url: String,
    pub trade_server_url: String,
    pub rpc_client  : Arc<Provider<Http>>,

    pub min_profit: U256,
    pub min_swap_amount: U256,

    pub block_info: BlockInfo,

    pub pools: Vec<H160>,
    pub pool_types: Vec<PoolType>,

    pub trade_info_map: Arc<RwLock<HashMap<(usize, bool), Vec<TradeInfo>>>>,
    pub pool_prices: Vec<U256>,

    pub chainlink_price: Arc<RwLock<U256>>,

    pub swap_topic_list: Vec<H256>,
    pub update_state_address_topic_list: Vec<(H160, H256)>,
    pub last_simulated_block_number: Arc<RwLock<U256>>,

    // pub flash_block_receiver: UnboundedReceiver<FlashblocksPayloadV1>,
    pub tx_sender: UnboundedSender<TypedTransaction>,
    pub last_seq_num: u64,
    pub test_mode: bool,
    pub gas_limit: u64,
    pub last_tx_time: u128,
    // pub last_tx_limit_price: i64
    pub step_count: u64,
    pub regression_count: u64,
    // pub simulate_interval: u64,
}

impl BaseStrategy {
    pub async fn new(
        rpc_url: String,
        ws_url: String,
        flashblocks_url: String,
        trade_server_url: String,
        rpc_client: Arc<Provider<Http>>,
        from_addr: H160,
        to_addr: H160,
        simulator_addr: H160,
        chain_id: u64,
        tx_sender: UnboundedSender<TypedTransaction>,
        gas_limit: u64,
        simulation_mode: bool,
        test_mode: bool,
        step_count: u64,
        regression_count: u64,
        min_profit: U256,
        min_swap_amount: U256,
    ) -> Self {
        let chain_id = chain_id;
        let from_addr = from_addr;
        let to_addr = to_addr;
        let swap_topic_list = vec![
            H256::from_str("0xc42079f94a6350d7e6235f29174924f928cc2ac818eb64fed8004e115fbcca67").unwrap(),
            H256::from_str("0x19b47279256b2a23a1665c810c8d55a1758940ee09377d4f8d26497a3577dc83").unwrap(),
        ];

            // let binance_price_reader = Arc::new(RwLock::new(U256::from(0)));
        // start_binance_collector(vec!["ETHUSDT".to_string()], binance_price_reader.clone()).await;

        let update_state_address_topic_list = vec![
            (H160::from_str("0x57d2d46Fc7ff2A7142d479F2f59e1E3F95447077").unwrap(), H256::from_str("0x0559884fd3a460db3073b7fc896cc77986f16e378210ded43186175bf646fc5f").unwrap()),
            (H160::from_str("0xDE4FB30cCC2f1210FcE2c8aD66410C586C8D1f9A").unwrap(), H256::from_str("0xb3e2773606abfd36b5bd91394b3a54d1398336c65005baf7bf7a05efeffaf75b").unwrap()),
            (H160::from_str("0xcEFC8B799a8EE5D9b312aeca73262645D664AaF7").unwrap(), H256::from_str("0xb3e2773606abfd36b5bd91394b3a54d1398336c65005baf7bf7a05efeffaf75b").unwrap()),
            (H160::from_str("0x7501bc8Bb51616F79bfA524E464fb7B41f0B10fB").unwrap(), H256::from_str("0xc42079f94a6350d7e6235f29174924f928cc2ac818eb64fed8004e115fbcca67").unwrap()),
        ];

        Self {
            rpc_url,
            rpc_client,
            ws_url,
            flashblocks_url,
            trade_server_url,
            chain_id,
            simulation_mode,
            from_addr,
            to_addr,
            simulator_addr,
            min_profit,
            min_swap_amount,
            block_info: BlockInfo::default(),
            pools: vec![],
            pool_types: vec![],
            trade_info_map: Arc::new(RwLock::new(HashMap::new())),
            chainlink_price: Arc::new(RwLock::new(U256::from(0))),
            pool_prices: vec![],

            last_simulated_block_number: Arc::new(RwLock::new(U256::from(0))),
            // block_number: U256::from(0),
            // block_timestamp: U256::from(0),
            // base_fee_per_gas: U256::from(0),
            swap_topic_list,
            update_state_address_topic_list,

            // flash_block_receiver,
            tx_sender,

            last_seq_num: 0,        
            test_mode,
            gas_limit,
            last_tx_time: 0,    
            step_count,
            regression_count,
            // simulate_interval,
            // last_tx_limit_price: 0
        }
    }

    pub async fn run(&mut self) {
        if self.simulation_mode {
            self.run_simulator().await;
        } else {
            self.run_executor().await;
        }
    }
        
    pub async fn run_simulator(&mut self) {
        let block_info_reader = start_block_collector(self.ws_url.clone()).await;
        let mut base_balance_list = vec![];
        let mut last_base_balance_update_block_number = U256::from(0);

        let subscribers = start_trade_server(self.trade_server_url.clone()).await;

        loop {
            let block_info = block_info_reader.read().await.clone();
    
            if block_info.number != self.block_info.number {
                println!("Block: {}", block_info.number);
                self.block_info = block_info;
    
                // Update base balance list
                if self.block_info.number > last_base_balance_update_block_number + 100 {            
                    last_base_balance_update_block_number = self.block_info.number;
                    let launcher = MSLauncher::new(self.to_addr, self.rpc_client.clone());
                    base_balance_list = launcher.get_base_balance_list().call().await.unwrap();
                }
    
                // Simulate trade
                let trade_info_list_new: Vec<TradeInfo> =
                    simulate_trade(
                        self.rpc_client.clone(),
                        self.rpc_url.as_str(),
                        block_info,
                        self.from_addr,
                        self.to_addr,
                        self.simulator_addr,
                        base_balance_list.clone(),
                        self.min_profit,
                        self.min_swap_amount).await;

                let trade_info_list_new_json = serde_json::to_value(trade_info_list_new).unwrap();
                broadcast_trade(subscribers.clone(), trade_info_list_new_json).await;
            }
            
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }



    pub async fn run_executor(&mut self) {
        let mut flash_block_receiver = 
            start_flash_block_collector(self.flashblocks_url.clone(), self.chain_id).await;        
        
        // Start trade collector
        self.trade_info_map = start_trade_collector(self.trade_server_url.clone()).await;

        let launcher = MSLauncher::new(self.to_addr, self.rpc_client.clone());
        let pool_count = launcher.get_pool_count().call().await.unwrap().as_u64() as usize;

        for i in 0..pool_count {
            let (pool, pool_type) = launcher.get_pool_info(i.into()).call().await.unwrap_or_default();

            self.pools.push(pool);
            self.pool_types.push(pool_type.into());
            self.pool_prices.push(U256::from(0));
        }

        loop {
            // Receive flashblock
            let flashblock = flash_block_receiver
                .recv()
                .await
                .expect("Failed to receive data from feed client");

            let seq_num = flashblock.index + flashblock.metadata.block_number * 100;
            if self.last_seq_num >= seq_num {
                continue;
            }

            self.last_seq_num = seq_num;

            let drift = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_millis() as i128
             + 2000 - self.block_info.timestamp.as_u128() as i128 * 1000;
            println!("Drift: {:?}", drift);

            // Process flashblock to get pool prices
            self.process_flash_block(flashblock).await;

            // Find profitable trade
            let (bid_prices, ask_prices, mut max_profit) = self.find_profitable_trade().await;
            if max_profit > U256::zero() {
                self.send_tx(bid_prices, ask_prices, max_profit).await;
            }

            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }

    async fn find_profitable_trade(&self) -> (Vec<U256>, Vec<U256>, U256) {
        let trade_info_map = self.trade_info_map.read().await.clone();
        let mut bid_prices = vec![U256::zero(); self.pools.len()];
        let mut ask_prices = vec![U256::zero(); self.pools.len()];
        let mut max_profit = U256::zero();

        for i in 0..self.pools.len() {
            let pool_price = self.pool_prices[i];
            // if pool_price == U256::from(0) {
            //     continue;
            // }

            for sell_base_token in [true, false] {
                if trade_info_map.contains_key(&(i, sell_base_token)) == false {
                    continue;
                }

                let trade_infos = trade_info_map.get(&(i, sell_base_token)).unwrap();
                for t in trade_infos.iter() {
                    println!("Pool: {}, {}, Delta: {:?}, Price: {:?}, Swap Amount: {:?}, Profit: {:?}",
                        t.pool_index,
                        if t.sell_base_token {"Bid"} else {"Ask"},
                        &format_ether(t.delta),
                        sqrt_price_x96_to_price(t.sqrt_price_x96, true, 18, 6),
                        &format_ether(t.swap_amount),
                        &format_ether(t.profit),
                    );

                    if pool_price == U256::from(0) {
                        continue;
                    }

                    if self.test_mode || (pool_price < t.sqrt_price_x96) == sell_base_token {
                        if sell_base_token {
                            bid_prices[i] = t.sqrt_price_x96;
                        } else {
                            ask_prices[i] = t.sqrt_price_x96;
                        }

                        if self.test_mode {
                            max_profit = WEI_IN_ETHER / 1000;
                        }
                                    
                        break;
                    }
                    max_profit = max_profit.max(t.profit);
                }
            }


            // if bid_prices[i] > U256::zero() && ask_prices[i] > U256::zero()
            {
                println!("Pool{i}: [{:.4}] < [{:.4}] < [{:.4}]", 
                    sqrt_price_x96_to_price(ask_prices[i], true, 18, 6).as_u64() as f64 / 100000000.0,
                    sqrt_price_x96_to_price(pool_price, true, 18, 6).as_u64() as f64 / 100000000.0,
                    sqrt_price_x96_to_price(bid_prices[i], true, 18, 6).as_u64() as f64 / 100000000.0);
            }
        }

        (bid_prices, ask_prices, max_profit)
    }

    // async fn process_binance_price(&mut self) {
    //     let binance_price = *self.binance_price_reader.read().await;
    //     if binance_price == U256::from(0) {
    //         return;
    //     }

    //     let sqrt_price_x96 = price_to_sqrt_price_x96(binance_price);
    //     *self.binance_price.write().await = sqrt_price_x96;
    //     // println!("Binance: {} - {sqrt_price_x96}", binance_price.as_u64() as f64 / 100000000.0);
    // }

    async fn process_flash_block(&mut self, flash_block: FlashblocksPayloadV1) {
        if flash_block.base.is_some() {
            let base = flash_block.base.unwrap();
            if self.block_info.timestamp != U256::from(base.timestamp) {
                self.block_info.number = U256::from(base.block_number);
                self.block_info.base_fee_per_gas = base.base_fee_per_gas;
                self.block_info.timestamp = U256::from(base.timestamp);

                println!("Block Number: {:?}, Base Fee: {:?}, Timestamp: {:?}", self.block_info.number, self.block_info.base_fee_per_gas, self.block_info.timestamp);
            }
        }

         // Flashblocks Price
         for (hash, receipt) in flash_block.metadata.receipts.as_object().unwrap() {
            for (key, value) in receipt.as_object().unwrap() {
                let log_receipts = serde_json::from_value::<LogReceipts>(value.clone()).unwrap_or_default();

                for log in log_receipts.logs {
                    if log.topics.is_empty() {
                        continue;
                    }

                    if self.pools.contains(&log.address) {
                        let pool_index = self.pools.iter().position(|p| p == &log.address).unwrap();
                        let topic0_index = self.swap_topic_list.iter().position(|t| t == &log.topics[0]);

                        if topic0_index.is_some() {
                            let pool_type = self.pool_types[pool_index];

                            println!("Pool Type: {:?}", pool_type);
                            let price = decode_price(log.data.to_string(), pool_type);

                            println!("Pool {pool_index} : {} - {price}", sqrt_price_x96_to_price(price, true, 18, 6));
                            self.pool_prices[pool_index] = price;
                        }
                    }

                    // if self.update_state_address_topic_list.contains(&(log.address, log.topics[0])) {
                    //     println!("Simulation Required: {:?}, {:?}", log.address, log.topics[0]);
                    //     *self.last_simulated_block_number.write().await = U256::from(0);

                    //     // Reset bid and ask prices if chainlink price is updated
                    //     if self.update_state_address_topic_list[0].0 == log.address {
                    //         // *self.limit_price_map.write().await.clear();
                    //         // *self.bid_prices.write().await = vec![U256::from(0); self.pools.len()];
                    //         // *self.ask_prices.write().await = vec![U256::from(0); self.pools.len()];
                    //     }
                    // }
                }
            }
        }
    }

    async fn send_tx(&mut self, bid_prices: Vec<U256>, ask_prices: Vec<U256>, max_profit: U256) {
        let mut gas_price = max_profit / 30000000;
        let base_price = U256::from(self.block_info.base_fee_per_gas) * 3 / 2;
        
        println!("Max Profit: {:?}, Gas Price: {:?} Mwei, Base Price: {:?} Mwei", 
            format_ether(max_profit), gas_price / U256::from(1000000), base_price / U256::from(1000000));

        if gas_price < base_price {
            gas_price = base_price;
        }

        let mut encoded: Vec<u8> = vec![];
        for i in 0..self.pools.len() {
            let bid_price = bid_prices[i];
            let ask_price = ask_prices[i];

            let mut bid_bytes = [0u8; 32];
            let mut ask_bytes = [0u8; 32];
            
            bid_price.to_big_endian(&mut bid_bytes);
            ask_price.to_big_endian(&mut ask_bytes);
            
            encoded.extend_from_slice(&bid_bytes[20..32]);
            encoded.extend_from_slice(&ask_bytes[20..32]);
        }

        let tx = TypedTransaction::Eip1559(
            Eip1559TransactionRequest::new()
                .from(self.from_addr)
                .to(self.to_addr)
                .value(U256::from(0))
                .data(encoded)
                .gas(U256::from(self.gas_limit))
                .max_fee_per_gas(gas_price)
                .max_priority_fee_per_gas(gas_price),
        );

        self.tx_sender.send(tx);
    }

}

async fn simulate_trade(
    rpc_client: Arc<Provider<Http>>,
    rpc_url: &str,
    block_info: BlockInfo,
    from_addr: H160,
    launcher_addr: H160,
    simulator_addr: H160,
    base_balance_list: Vec<U256>,
    min_profit: U256,
    min_swap_amount: U256,
) -> Vec<TradeInfo>
{
    let shared_backend = create_shared_backend(
        rpc_url.clone(),
        Some(block_info.number.as_u64().into())).await.unwrap();
    let provider = Arc::new(get_provider(rpc_url.clone()).await.unwrap());

    let mut join_set = JoinSet::new();

    // let launcher_addr = H160::from_str("0x558b6738759a5DBa97aab14CE602b8d20ba05087").unwrap();
    // let simulator_addr = H160::from_str("0x6F804aeE9d94DcE18B874defbc5DFC0334C14c99").unwrap();
    let weth_addr = H160::from_str("0x4200000000000000000000000000000000000006").unwrap();
    let weth_balance_slot = calculate_balance_slot(simulator_addr, 3);
    let weth_balance_value = H256::from_str("0x00000000000000000000000000000000000000000000d3c21bcecceda1000000").unwrap();
    let usdc_addr = H160::from_str("0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913").unwrap();
    let usdc_balance_slot = calculate_balance_slot(simulator_addr, 9);
    let usdc_balance_value = H256::from_str("0x0000000000000000000000000000000000000000000000000de0b6b3a7640000").unwrap();                
    let pool_count = base_balance_list.len();

    let storage_changes = vec![
        (weth_addr, weth_balance_slot, weth_balance_value),
        (usdc_addr, usdc_balance_slot, usdc_balance_value)];

    // Clone values that need to be moved into spawned tasks
    let storage_changes_clone = storage_changes.clone();
    let base_balance_list_clone = base_balance_list.clone();

    for sell_base_token in [true, false] {                    
        for pool_index in 0..pool_count {
            let shared_backend = shared_backend.clone();
            let rpc_client = rpc_client.clone();
            let provider = provider.clone();
            let storage_changes = storage_changes_clone.clone();
            let base_balance_list = base_balance_list_clone.clone();
            
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
                    base_balance_list[pool_index] / 3).await
            });
        }
    }

    let trade_info_list: Vec<TradeInfo> = join_set.join_all().await.into_iter().flatten().collect();
    trade_info_list
}
