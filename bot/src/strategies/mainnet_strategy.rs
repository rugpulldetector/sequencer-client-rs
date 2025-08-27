use tokio::{sync::mpsc::{UnboundedSender}};
use ethers::{types::{transaction::eip2718::TypedTransaction, H160, H256, U256}, utils::{keccak256, WEI_IN_ETHER}};
use ethers_providers::{Http, Provider};
use tokio::sync::RwLock;
use std::{str::FromStr, sync::Arc};

use crate::{abi::{MSLauncherMainnet, MSLauncherRouterMainnet, MSSimulatorMainnet, IERC20}, collectors::{block_collector::{start_block_collector, BlockInfo}}, types::{decode_price, price_to_sqrt_price_x96, sqrt_price_x96_to_price, FlashblocksPayloadV1, LogReceipts, PoolType}};

pub struct MainnetStrategy {

    pub from_addr: H160,
    pub to_addr: H160,
    pub launcher_addr: H160,
    pub simulator_addr: H160,

    pub rpc_client  : Arc<Provider<Http>>,
    
    pub mevshare_url: String,
    
    pub pools: Vec<H160>,
    pub target_pools: Vec<H160>,

    pub weth_balances: Arc<RwLock<Vec<U256>>>,
    pub bid_prices: Arc<RwLock<Vec<U256>>>,
    pub ask_prices: Arc<RwLock<Vec<U256>>>,    
    pub pool_prices: Arc<RwLock<Vec<U256>>>,
    pub chainlink_price: Arc<RwLock<U256>>,

    pub block_info: BlockInfo,
    pub block_info_reader: Arc<RwLock<BlockInfo>>,

    pub tx_sender: UnboundedSender<(TypedTransaction, Vec<H160>, U256)>,
    pub test_mode: bool,
    pub gas_limit: u64,
    pub step_count: u64,
    pub regression_count: u64,
    pub swap_count: usize,
}

impl MainnetStrategy {
    pub async fn new(
        rpc_client: Arc<Provider<Http>>,
        block_ws_url: String,
        mevshare_url: String,
        from_addr: H160,
        to_addr: H160,
        simulator_addr: H160,
        tx_sender: UnboundedSender<(TypedTransaction, Vec<H160>, U256)>,
        gas_limit: u64,
        test_mode: bool,
        step_count: u64,
        regression_count: u64
    ) -> Self {

        let block_info_reader = start_block_collector(block_ws_url).await;
        let block_info = block_info_reader.read().await.clone();

        Self {  
            rpc_client,
            block_info,
            block_info_reader,
            mevshare_url,
            from_addr,
            to_addr,
            launcher_addr: H160::from_str("0xa50d6ac929F1E7BA5d520662dBbF1Cb5D87E00d8").unwrap(),
            simulator_addr,
            pools: vec![],
            target_pools: vec![],
            weth_balances: Arc::new(RwLock::new(vec![])),
            
            bid_prices: Arc::new(RwLock::new(vec![])),
            ask_prices: Arc::new(RwLock::new(vec![])),
            chainlink_price: Arc::new(RwLock::new(U256::from(0))),
            pool_prices: Arc::new(RwLock::new(vec![])),

            tx_sender,

            test_mode,
            gas_limit,
            step_count,
            regression_count,
            swap_count: 50
        }
    }

    pub async fn run(&mut self) {
        let launcher = MSLauncherMainnet::new(
            H160::from_str("0xa50d6ac929F1E7BA5d520662dBbF1Cb5D87E00d8").unwrap(), self.rpc_client.clone());
        let weth_addr = H160::from_str("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2").unwrap();
        let weth = IERC20::new(weth_addr, self.rpc_client.clone());
        let pool_count = launcher.get_pool_count().call().await.unwrap();

        for i in 0..pool_count.as_u64() as usize {
            let (pool, pool_type) = launcher.get_pool_info(U256::from(i)).call().await.unwrap();
            let weth_balance = if pool_type != PoolType::UniswapV4 as u8 {
                self.target_pools.push(pool);
                weth.balance_of(pool).call().await.unwrap()
            } else {
                self.target_pools.push(H160::from_str("0x000000000004444c5dc75cB358380D2e3dE08A90").unwrap());
                WEI_IN_ETHER * 800
            };

            self.pools.push(pool);
            self.bid_prices.write().await.push(U256::from(0));
            self.ask_prices.write().await.push(U256::from(0));
            self.pool_prices.write().await.push(U256::from(0));
            self.weth_balances.write().await.push(weth_balance);
        }

        // self.start_mevshare_loop().await;

        loop {
            let block_info = self.block_info_reader.read().await.clone();
            if block_info.number == self.block_info.number {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                continue;
            }

            self.block_info = block_info;
            println!("Block Number: {:?}, Current Base Fee: {:?}, Next Base Fee: {:?}",
                block_info.number,
                block_info.base_fee_per_gas,
                block_info.next_base_fee);

            for i in 0..self.pools.len() {
                let sqrt_price_x96 = launcher.get_sqrt_price_x96(U256::from(i)).call().await.unwrap();
                self.pool_prices.write().await[i] = sqrt_price_x96;
            }
            
            // self.simulate().await;
            self.send_tx().await;
        }
    }
/*
    async fn start_mevshare_loop(&mut self) {
        let mevshare_url = self.mevshare_url.clone();
        let pools = self.pools.clone();
        let bid_prices = self.bid_prices.clone();
        let ask_prices = self.ask_prices.clone();
        let block_number = self.block_number.clone();

        tokio::spawn(async move {
            let pool_manager = H160::from_str("0x000000000004444c5dc75cB358380D2e3dE08A90").unwrap();
            let swap_topic = H256::from_str("0xc42079f94a6350d7e6235f29174924f928cc2ac818eb64fed8004e115fbcca67").unwrap();
            let swap_topic_v4 = H256::from_str("0x40e9cecb9f5f1f1c5b9c97dec2917b7ee92e57ba5563708daca94dd84ad7112f").unwrap();
            let bunni_pool_id = H256::from_str("0x9148f00424c4b40a9ec4b03912f091138e9e91a60980550ed97ed7f9dc998cb5").unwrap();

            let mut mevshare_receiver = start_mevshare_collector(mevshare_url).await;
            while let Some(event) = mevshare_receiver.recv().await {
                for log in event.logs {
                    if pools.contains(&log.address) && log.topics[0] == swap_topic {
                        let pool_index = pools.iter().position(|p| p == &log.address).unwrap();
                        if log.data.len() > 0 {
                            let price = decode_price(log.data.to_string(), PoolType::UniswapV3);
                            let bid_price = bid_prices.read().await[pool_index];
                            let ask_price = ask_prices.read().await[pool_index];

                            if price > U256::from(0) && bid_price > U256::from(0) && ask_price > U256::from(0) {
                                println!("V3 {pool_index} : {} -  {} - {}", 
                                WEI_IN_ETHER * WEI_IN_ETHER * 10000 / sqrt_price_x96_to_price(bid_price),
                                WEI_IN_ETHER * WEI_IN_ETHER * 10000 / sqrt_price_x96_to_price(price),
                                WEI_IN_ETHER * WEI_IN_ETHER * 10000 / sqrt_price_x96_to_price(ask_price));
                            }
                        } else {
                            println!("Pool {pool_index} - Empty data");
                        }
                    }

                    if 
                        log.address == pool_manager &&
                        log.topics.len() > 1 &&
                        log.topics[0] == swap_topic_v4
                        // log.topics[1] == bunni_pool_id &&
                        // log.data.len() > 0
                    {
                        // let price = decode_price(log.data.to_string(), PoolType::UniswapV4);
                        // if price > U256::from(0) {
                        //     println!("V4  : {:x} - {}",
                        //     log.topics[1],
                        //     sqrt_price_x96_to_price(price));
                        // }
                        
                        if log.topics[1] == bunni_pool_id {
                            println!("{} - Bunni Pool : {:x} - {:?}", *block_number.read().await, event.hash, log.data);
                        }
                    }
                }
            }
        });
    }

    async fn simulate(&mut self) {
        let weth_balances = self.weth_balances.clone();
        let chainlink_price = self.chainlink_price.clone();
        let min_profit = U256::zero();
        let weth_addr = H160::from_str("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2").unwrap();
        let weth_balance_slot = 3;
        let weth_balance_value = H256::from_str("0x00000000000000000000000000000000000000000000d3c21bcecceda1000000").unwrap();
        let usdc_addr = H160::from_str("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48").unwrap();
        let usdc_balance_slot = 9;
        let usdc_balance_value = H256::from_str("0x0000000000000000000000000000000000000000000000000de0b6b3a7640000").unwrap();
        let pool_count = self.pools.len();

        let block_number = *self.block_number.clone().read().await;
        println!("Simulation Block Number: {:?}", block_number);

        let mut set: JoinSet<(Result<(usize, U256, U256), ()>)> = JoinSet::new();

        for pool_index in 0..pool_count {
            // for swap_index in 0..self.swap_count
            {
                let simulator_addr = self.simulator_addr.clone();
                let launch_addr = self.launcher_addr.clone();
                let pools = self.pools.clone();
                let step_count = self.step_count.clone();
                let swap_amount = WEI_IN_ETHER * 2;
                // let swap_amount = WEI_IN_ETHER * (swap_index + 1);
                let rpc_client = self.rpc_client.clone();
                let step_count = U256::from(step_count);
                let start_step = weth_balances.read().await[pool_index] / step_count / 2;

                let mut state = spoof::state();
                state.account(weth_addr).store(Self::calculate_balance_slot
                    (simulator_addr.clone(), weth_balance_slot), weth_balance_value);
                state.account(usdc_addr).store(Self::calculate_balance_slot
                    (simulator_addr.clone(), usdc_balance_slot), usdc_balance_value);
                
                set.spawn(async move {
                    let simulator = MSSimulatorMainnet::new(simulator_addr.clone(), rpc_client.clone());
                    let mut bid_price = U256::zero();
                    let mut ask_price = U256::zero();

                    let directions = [true, false];
                    for direction in directions {
                        let mut start = -I256::from(WEI_IN_ETHER.as_u128() * 10);
                        let mut step = start_step;

                        while step * 10000 > WEI_IN_ETHER {
                            let tx = simulator.get_limit_price(
                                launch_addr, 
                                U256::from(pool_index), 
                                direction,
                                start,
                                step,
                                step_count,
                                swap_amount,
                                min_profit).tx;

                            for retry in 0..10 {
                                let raw_call = rpc_client.call_raw(&tx)
                                .state(&state)
                                .block(BlockId::Number(BlockNumber::Number(block_number.as_u64().into())));

                                match raw_call.await {
                                    Ok(rslt) => {
                                        let (price, cl_price, delta_amount, profit): (U256, U256, I256, U256) = 
                                                <(U256, U256, I256, U256)>::decode(&rslt).expect("decode failed");

                                        // println!("Price: {:?}, Delta Amount: {:?}, Profit: {:?}", price, delta_amount, profit);
                
                                        start = delta_amount - I256::from(step.as_u128());
                                        step = step / 10;

                                        // let mut chainlink_price = chainlink_price.clone();
                                        // *chainlink_price.write().await = cl_price;

                                        if direction {
                                            bid_price = price;
                                        } else {
                                            ask_price = price;
                                        }
                                        break;                                       
                                    },
                                    Err(e) => {
                                        println!("Error: {:?}", e);
                                    }
                                }
                                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                            }
                        }
                    }

                    Ok((pool_index, bid_price, ask_price))
                });
            }
        }

        while let Some(res) = set.join_next().await {
            if let Ok((pool_index, bid_price, ask_price)) = res.unwrap() {
                self.bid_prices.write().await[pool_index] = bid_price;
                self.ask_prices.write().await[pool_index] = ask_price;
            }
        }

        for pool_index in 0..pool_count {
            // for swap_index in 0..self.swap_count
            {      
                let bid_price = self.bid_prices.read().await[pool_index];
                let ask_price = self.ask_prices.read().await[pool_index];

                if bid_price > U256::from(0) && ask_price > U256::from(0) {
                    println!("Pool: {pool_index}, Bid Price: {:?}, Ask Price: {:?}", 
                        WEI_IN_ETHER * WEI_IN_ETHER * 10000 / sqrt_price_x96_to_price(bid_price),
                        WEI_IN_ETHER * WEI_IN_ETHER * 10000 / sqrt_price_x96_to_price(ask_price));
                }
            }
        }

    }
 */
    async fn send_tx(&mut self) {
        let router = MSLauncherRouterMainnet::new(self.to_addr, self.rpc_client.clone());
        let mut tx = router.launch(
            self.bid_prices.read().await.clone(),
            self.ask_prices.read().await.clone()).tx;

        tx.set_from(self.from_addr);
        tx.set_to(self.to_addr);
        tx.set_value(U256::from(10));
        tx.set_gas(U256::from(self.gas_limit));
        tx.set_gas_price(self.block_info.next_base_fee * 5 / 4);   

        let next_block_number=  self.block_info.number + 2;
        self.tx_sender.send((tx, self.target_pools.clone(), next_block_number));
    }


    fn calculate_balance_slot(holder_address: H160, mapping_slot: u64) -> H256 {        
        // Encode the address and slot in the standard Solidity format
        let encoded = ethers::abi::encode(&[
            ethers::abi::Token::Address(holder_address),
            ethers::abi::Token::Uint(mapping_slot.into()),
        ]);
        
        // Calculate the keccak256 hash
        let hash = keccak256(encoded);
        
        // Convert to H256
        H256::from_slice(&hash)
    }

}