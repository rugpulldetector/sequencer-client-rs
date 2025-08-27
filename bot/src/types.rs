use alloy_primitives::TxKind;
use ethers::abi::decode;
use ethers::abi::ParamType;
use ethers::abi::Tokenizable;
use ethers::types::transaction::eip2718::TypedTransaction;
use ethers::types::BlockId;
use ethers::types::Bytes;
use ethers::types::I256;
use ethers::utils::format_ether;
use ethers::utils::keccak256;
use ethers::utils::WEI_IN_ETHER;
use ethers_providers::spoof;
use grouping_by::GroupingBy;
use jsonrpsee::core::client::SubscriptionClientT;
use jsonrpsee::rpc_params;
use jsonrpsee::ws_client::WsClientBuilder;
use jsonrpsee_ws_server::RpcModule;
use jsonrpsee_ws_server::SubscriptionSink;
use jsonrpsee_ws_server::WsServerBuilder;
use rayon::result;
use revm_trace::create_evm_from_shared_backend;
use revm_trace::evm::builder::get_provider;
use revm_trace::evm::NoOpInspector;
use revm_trace::revm::bytecode::eof::printer::print;
use revm_trace::revm::context::result::ExecutionResult;
use revm_trace::types::AnyNetworkProvider;
use revm_trace::types::ArcAnyNetworkProvider;
use revm_trace::types::StateOverride;
use revm_trace::SharedBackend;
use revm_trace::SimulationBatch;
use revm_trace::SimulationTx;
use revm_trace::TransactionTrace;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use anyhow::Result;
use async_trait::async_trait;
use ethers::types::Log;
use ethers::types::H160;
use ethers::types::H256;
use ethers::types::U256;
use ethers::types::Block;
use tokio::sync::broadcast::Sender;
use std::collections::HashMap;
use std::pin::Pin;
use tokio_stream::Stream;
use tokio_stream::StreamExt;
use anyhow::anyhow;
use crate::collectors::block_collector::BlockInfo;
use crate::abi::MSLauncher;

use ethers_providers::{Http, Provider, RawCall};
use tokio::sync::RwLock;
use std::{str::FromStr, sync::Arc};
use ethers::abi::AbiDecode;


#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct TradeInfo {
    pub launcher_addr: H160,
    pub pool_index: usize,
    pub sell_base_token: bool,
    pub start_delta: U256,
    pub delta: U256,
    pub sqrt_price_x96: U256,
    pub trade_price: U256,
    pub deviation_bps: U256,
    pub swap_amount: U256,
    pub profit: U256,
    pub gas_used: U256,
}

/// Represents the modified portions of an execution payload within a flashblock.
/// This structure contains only the fields that can be updated during block construction,
/// such as state root, receipts, logs, and new transactions. Other immutable block fields
/// like parent hash and block number are excluded since they remain constant throughout
/// the block's construction.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ExecutionPayloadFlashblockDeltaV1 {
    pub transactions: Vec<Bytes>
}

/// Represents the base configuration of an execution payload that remains constant
/// throughout block construction. This includes fundamental block properties like
/// parent hash, block number, and other header fields that are determined at
/// block creation and cannot be modified.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ExecutionPayloadBaseV1 {
    #[serde(with = "alloy_serde::quantity")]
    pub block_number: u64,
    /// The gas limit of the block.
    #[serde(with = "alloy_serde::quantity")]
    pub gas_limit: u64,
    /// The timestamp of the block.
    #[serde(with = "alloy_serde::quantity")]
    pub timestamp: u64,
    /// The base fee per gas of the block.
    pub base_fee_per_gas: U256,
}


/// Represents the base configuration of an execution payload that remains constant
/// throughout block construction. This includes fundamental block properties like
/// parent hash, block number, and other header fields that are determined at
/// block creation and cannot be modified.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct MetadataV1 {
    #[serde(with = "alloy_serde::quantity")]
    pub block_number: u64,
    pub receipts: Value
}


/// Represents the base configuration of an execution payload that remains constant
/// throughout block construction. This includes fundamental block properties like
/// parent hash, block number, and other header fields that are determined at
/// block creation and cannot be modified.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct LogReceipts {
    pub logs: Vec<LogItem>
}

/// A log produced by a transaction.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogItem {
    /// H160. the contract that emitted the log
    pub address: H160,
    pub topics: Vec<H256>,
    pub data: Bytes,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct FlashblocksPayloadV1 {
    /// The index of the flashblock in the block
    pub index: u64,
    /// The base of the flashblock
    pub base: Option<ExecutionPayloadBaseV1>,
    /// The delta/diff containing modified portions of the execution payload
    pub diff: ExecutionPayloadFlashblockDeltaV1,
    /// Additional metadata associated with the flashblock
    pub metadata: MetadataV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoolType {
    UniswapV3,
    PancakeV3,
    Aerodrome,
    UniswapV4,
}

impl From<u8> for PoolType {
    fn from(value: u8) -> Self {
        match value {
            0 => PoolType::UniswapV3,
            1 => PoolType::PancakeV3,
            2 => PoolType::Aerodrome,
            3 => PoolType::UniswapV4,
            _ => panic!("Invalid pool type: {}", value),
        }
    }
}

pub fn decode_price(data_hex: String, pool_type: PoolType) -> U256 {
    // Strip '0x' prefix and decode hex to bytes
    let hex_str = data_hex.trim_start_matches("0x");
    let data_bytes = hex::decode(hex_str).expect("Invalid hex data");

    // Define expected parameter types
    let param_types = if pool_type == PoolType::PancakeV3 {
        vec![
            ParamType::Int(256),  // amount0 (int256)
            ParamType::Int(256),  // amount1 (int256)
            ParamType::Uint(160), // sqrtPriceX96 (uint160)
            ParamType::Uint(128), // liquidity (uint128)
            ParamType::Int(24),   // tick (int24)
            ParamType::Uint(128), // protocolFee1 (uint128)
            ParamType::Uint(128), // protocolFee2 (uint128)
        ]
    } else if pool_type == PoolType::UniswapV4 {
        vec![
            ParamType::Int(128),  // amount0 (int128)
            ParamType::Int(128),  // amount1 (int128)
            ParamType::Uint(160), // sqrtPriceX96 (uint160)
            ParamType::Uint(128), // liquidity (uint128)
            ParamType::Int(24),   // tick (int24)
            ParamType::Uint(24), // fee (uint24)
        ]
    } else {
        vec![
            ParamType::Int(256),  // amount0 (int256)
            ParamType::Int(256),  // amount1 (int256)
            ParamType::Uint(160), // sqrtPriceX96 (uint160)
            ParamType::Uint(128), // liquidity (uint128)
            ParamType::Int(24),   // tick (int24)
        ]
    };

    // Decode into tokens
    let tokens = decode(&param_types, &data_bytes).expect("ABI decoding failed");

    U256::from_token(tokens[2].clone()).unwrap()

    // sqrt_price_x96_to_price(U256::from_token(tokens[2].clone()).unwrap())
}


pub fn sqrt_price_x96_to_price(sqrt_price_x96: U256, zero_for_one: bool, decimal0: u8, decimal1: u8) -> U256 {
    let q96 = U256::from(2).pow(96.into());
    let x = U256::from(10).pow(((decimal0 - decimal1 + 8) / 2).into());
    let price = sqrt_price_x96 * x / q96 * sqrt_price_x96 * x / q96;

    if price == U256::from(0) {
        return price;
    }

    if zero_for_one {
        price
    } else {
        U256::from(10).pow(40.into()) / price
    }
}


pub fn price_to_sqrt_price_x96(price: U256) -> U256 {
    let q96 = U256::from(2).pow(96.into());
    let x = U256::from(10).pow(10.into());
    let sqrt_price_x96 = (price * q96 * q96 / x / x).integer_sqrt();
    sqrt_price_x96
}

fn H256_to_U256(h256: H256) -> alloy_primitives::Uint<256, 4> {
    alloy_primitives::Uint::from_be_bytes(h256.0.into())
}        


pub fn calculate_balance_slot(holder_address: H160, mapping_slot: u64) -> H256 {        
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

pub fn to_spoof_state(storage_changes: &Vec<(H160, H256, H256)>) -> spoof::State {
    let mut state = spoof::state();
    for (addr, slot, value) in storage_changes {
        state.account(addr.clone()).store(slot.clone(), value.clone());
    }
    state
}

pub fn to_state_override(storage_changes: &Vec<(H160, H256, H256)>) -> StateOverride {
    let mut state_override = StateOverride {
        storages: HashMap::new(),
        balances: HashMap::new(),
    };

    for (addr, slot, value) in storage_changes {
        state_override.storages.insert(addr.0.into(), vec![(H256_to_U256(slot.clone()), H256_to_U256(value.clone()))]);
    }
    state_override
}


pub async fn simulate_tx_with_revm(
    shared_backend: SharedBackend,
    provider: ArcAnyNetworkProvider,
    from_addr: H160,
    txs: Vec<TypedTransaction>,
    is_stateful: bool,
    state_override: StateOverride,
) -> Vec<Option<Vec<u8>>> {
    let mut shared_evm = 
        create_evm_from_shared_backend(
            shared_backend.clone(),
            &provider,
            NoOpInspector::default(),
        ).await.unwrap();

    let mut batch = SimulationBatch {
        transactions: vec![],
        is_stateful: is_stateful,
        overrides: Some(state_override),
    };

    for tx in txs {
        let simulation_tx = SimulationTx {
            caller: from_addr.0.into(),
            transact_to: TxKind::Call(tx.to_addr().unwrap().0.into()),
            value: alloy_primitives::Uint::from(0),
            data: tx.data().unwrap().0.clone().into(),
        };

        batch.transactions.push(simulation_tx); 
    }

    // println!("Batch: {:?}", batch);

    let mut results = vec![];

    for trace in shared_evm.trace_transactions(batch) {
        match trace {
            Ok(trace) => {
                match trace.0 {
                    ExecutionResult::Success { reason, gas_used, gas_refunded, logs, output } => {
                        let bytes = output.clone().into_data().as_ref().to_vec();
                        results.push(Some(bytes));
                        continue;
                    }
                    ExecutionResult::Revert { gas_used, output } => {
                        println!("Revert: {:?}, {:?}", output, gas_used);
                    }
                    ExecutionResult::Halt { reason, gas_used } => {
                        println!("Halt: {:?}, {:?}", reason, gas_used);
                    }
                }
            }
            Err(e) => {
                println!("Error: {:?}", e);
            }
        }

        results.push(None);
    }

    results
}


pub async fn find_best_trade_list(
    shared_backend: SharedBackend,
    provider: ArcAnyNetworkProvider,
    rpc_client: Arc<Provider<Http>>,
    from_addr: H160,
    launcher_addr: H160,
    simulator_addr: H160,
    min_profit: U256,
    min_swap_amount: U256,
    storage_changes: Vec<(H160, H256, H256)>,
    pool_index: usize,
    sell_base_token: bool,
    start_delta: U256,
) -> Vec<TradeInfo> {
    let mut max_delta = start_delta;
    let mut min_delta = U256::from(0);
    let mut delta= U256::from(0);
    let state_override = to_state_override(&storage_changes);
    let simulator = MSLauncher::new(simulator_addr, rpc_client.clone());

    // First step: find the optimal delta from which swap is profitable
    while (max_delta - min_delta) * 1000 > WEI_IN_ETHER {
        let new_delta = (max_delta + min_delta) / 2;

        let mut tx = simulator.simulate_trade(
                    launcher_addr,
                    U256::from(pool_index),
                    sell_base_token,
                    I256::from(new_delta.as_u128()),
                    min_swap_amount,
                ).tx;

        tx.set_gas(U256::from(10000000));

        let result = &simulate_tx_with_revm(
            shared_backend.clone(),
            provider.clone(),
            from_addr,
            vec![tx],   
            false,
            state_override.clone(),
        ).await[0];
        if let Some(result) = result {
            let (new_sqrt_price_x96, new_profit, new_gas_used): (U256, U256, U256) = 
                        <(U256, U256, U256)>::decode(result.as_slice()).expect("decode failed");

            if new_profit > min_profit {
                max_delta = new_delta;
                delta = new_delta;

                if pool_index == 0 && sell_base_token {
                    println!("Step 1: Price: {:?}, Profit: {:?}, Delta: {:?}",
                        sqrt_price_x96_to_price(new_sqrt_price_x96, true, 18, 6),
                        &format_ether(new_profit),
                        &format_ether(delta));
                }
                    
                continue;
            }
        }

        min_delta = new_delta;
    }

    // Second step: starting from swap amount, find the next profitable delta, swap amount, and sqrt_price_x96
    let regression_count = 10;
    let regression_step = (WEI_IN_ETHER * U256::from(3) / U256::from(2)).min(start_delta / 100);
    let mut txs = vec![];
    let mut delta_list = vec![];
    let mut sqrt_price_x96_list = vec![U256::from(0); regression_count];
    let mut swap_amount_list = vec![U256::from(0); regression_count];
    let mut profit_list = vec![U256::from(0); regression_count];
    let mut gas_used_list = vec![U256::from(0); regression_count];

    for i in 0..regression_count {
        let tx = simulator.simulate_price_and_amount(
            launcher_addr,
            U256::from(pool_index),
            sell_base_token,
            I256::from(delta.as_u128())).tx;

        delta_list.push(delta);
        txs.push(tx);

        delta = delta + regression_step;
    }

    let results = simulate_tx_with_revm(
        shared_backend.clone(),
        provider.clone(),
        from_addr,
        txs,   
        false,
        state_override.clone(),
    ).await;

    let mut txs2 = vec![];
    let mut tx2_indices = vec![];

    for i in 0..regression_count {
        let mut sqrt_price_x96 = U256::from(0);
        let mut swap_amount = U256::from(0);

        match results[i].clone() {
            Some(result) => {
                (sqrt_price_x96, swap_amount) = 
                    <(U256, U256)>::decode(result).expect("decode failed");

                let tx = simulator.simulate_trade(
                        launcher_addr,
                        U256::from(pool_index),
                        sell_base_token,
                        I256::from(delta.as_u128()),
                        swap_amount).tx;

                txs2.push(tx);
                tx2_indices.push(i);
            }
            None => {
                println!("No result for index: {}", i);
            }
        }
        
        sqrt_price_x96_list[i] = sqrt_price_x96;
        swap_amount_list[i] = swap_amount;
    }

    // Third step
    let results2 = simulate_tx_with_revm(
        shared_backend.clone(),
        provider.clone(),
        from_addr,
        txs2,   
        false,
        state_override.clone(),
    ).await;


    for (result, i) in results2.iter().zip(tx2_indices.iter()) {
        if let Some(result) = result {
            let (sqrt_price_x96, profit, gas_used): (U256, U256, U256) = 
                    <(U256, U256, U256)>::decode(result).expect("decode failed");

            profit_list[*i] = profit;
            gas_used_list[*i] = gas_used;
        }
    }

    let mut trade_info_list = vec![];

    // Last step
    for i in 0..regression_count {
        // let gas_price = profit_list[i] / gas_used_list[i];

        if profit_list[i] > min_profit && swap_amount_list[i] > min_swap_amount {
            let trade_info = TradeInfo {
                launcher_addr: launcher_addr,
                pool_index: pool_index,
                sell_base_token: sell_base_token,
                start_delta: delta_list[i],
                delta: delta_list[i],
                sqrt_price_x96: sqrt_price_x96_list[i],
                trade_price: U256::from(0),
                deviation_bps: U256::from(0),
                swap_amount: swap_amount_list[i],
                profit: profit_list[i],
                gas_used: gas_used_list[i],
            };

            trade_info_list.push(trade_info);
        }
    }
    
    trade_info_list
}


pub async fn start_trade_server(trade_server_url: String) -> Arc<RwLock<Vec<SubscriptionSink>>> {
    let subscribers: Arc<RwLock<Vec<SubscriptionSink>>> = Arc::new(RwLock::new(Vec::new()));
    let subscribers_clone = subscribers.clone();
    println!("Starting trade server: {}", trade_server_url);

    tokio::spawn(async move {
        let server = WsServerBuilder::default().build(trade_server_url).await.unwrap();
        let mut rpc = RpcModule::new(subscribers_clone.clone());
    
        // Clients call this to subscribe
        rpc.register_subscription(
            "subscribe_trade",     // subscription method name
            "trade",               // notification name
            "unsubscribe_trade",   // unsubscription method
            move |_, mut pending, ctx|  {

                println!("Incoming connection");

                // Use an async block inside spawn_local for clean async handling
                tokio::spawn(async move {
                    // Accept the subscription
                    if let Err(e) = pending.accept() {
                        eprintln!("Failed to accept subscription: {:?}", e);
                        return;
                    }

                    // Push the pending subscription into the shared vector
                    ctx.write().await.push(pending);
                });

                Ok(())
            },
        ).unwrap();

        let server_handle = server.start(rpc).unwrap();
        server_handle.await;
    });        

    subscribers
}

pub async fn broadcast_trade(subscribers: Arc<RwLock<Vec<SubscriptionSink>>>, trade: serde_json::Value) {
    let mut locked = subscribers.write().await;

    // Iterate over all sinks and try to send
    let mut i = 0;
    while i < locked.len() {
        let sink = &mut locked[i];
        if let Err(_) = sink.send(&trade) {
            // Remove disconnected clients
            locked.remove(i); // This is safe because we're not modifying the read guard
        } else {
            i += 1;
        }
    }
}

pub async fn start_trade_collector(trade_server_url: String) -> Arc<RwLock<HashMap<(usize, bool), Vec<TradeInfo>>>> {
    let trade_info_map = Arc::new(RwLock::new(HashMap::new()));
    let trade_info_map_clone = trade_info_map.clone();

    // println!("Subscribed to {:?}", sub);

    tokio::spawn(async move {
        println!("Connecting to trade collector: {}", trade_server_url);

        let client = WsClientBuilder::default()
            .build(format!("ws://{}", trade_server_url.clone()))
            .await.unwrap();
    
        let mut sub = client
        .subscribe(
            "subscribe_trade",
            rpc_params![],
            "unsubscribe_trade",
        )
        .await.unwrap();

        // Listen for notifications
        while let Some(trade) = sub.next().await {   
            let trade_list: serde_json::Value = trade.unwrap();
            let trade_list = serde_json::from_value::<Vec<TradeInfo>>(trade_list.clone()).unwrap();
            let mut trade_info_map = HashMap::new();

            for (key, mut trade_infos) in trade_list.iter().grouping_by(|t| (t.pool_index, t.sell_base_token)) {
                trade_infos.sort_by(|a, b| a.delta.cmp(&b.delta));                        
                trade_info_map.insert(key, trade_infos.clone().iter().map(|t| *t.clone()).collect::<Vec<_>>());
            }
            
            *trade_info_map_clone.write().await = trade_info_map;
            println!("Received trade: {:?}", trade_list.len());
        }
    });

    trade_info_map
}
