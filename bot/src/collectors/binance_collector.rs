
use crypto_ws_client::{BinanceSpotWSClient, WSClient};
use ethers::types::U256;
use serde::Deserialize;
use tokio::sync::RwLock;
use std::{collections::HashMap, sync::Arc};

#[derive(Debug, Deserialize)]
pub struct BinanceMessage {
    pub data: BinanceData
}

#[derive(Debug, Deserialize)]
pub struct BinanceData {
    pub s: String,
    pub p: String,
}

pub async fn start_binance_collector(symbols: Vec<String>) -> Arc<RwLock<HashMap<String, U256>>> {
    let reader = Arc::new(RwLock::new(HashMap::new()));
    let reader_clone = reader.clone();

    let (sender_ws, receiver_ws) = std::sync::mpsc::channel();

    tokio::spawn(async move {
        let ws_client = BinanceSpotWSClient::new(sender_ws.clone(), None).await;
        println!("[MS] Connected to Binance");

        ws_client.subscribe_trade(&symbols).await;
        println!("[MS] Subscribed to {:?}", symbols);

        ws_client.run().await;
        ws_client.close().await;
    });

    tokio::spawn(async move {
        loop {
            while let Ok(msg) = receiver_ws.recv() {
                let binance_message = serde_json::from_str::<BinanceMessage>(&msg).unwrap();
                let binance_price = (binance_message.data.p.parse::<f64>().unwrap_or(0.0) * 100000000.0) as u64;
                if binance_price != 0 {
                    let mut binance_price_map = reader_clone.write().await;
                    binance_price_map.insert(binance_message.data.s, U256::from(binance_price));
                }
            }
        }
    });

    reader
}