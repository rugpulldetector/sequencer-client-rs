use ethers::types::U256;
use ethers_providers::{Middleware, Provider, Ws};
use tokio::sync::RwLock;
use std::sync::Arc;
use tokio_stream::StreamExt;


/// A new block event, containing the block number and hash.
#[derive(Debug, Clone, Default, Copy)]
pub struct BlockInfo {
    pub number: U256,
    pub gas_used: U256,
    pub gas_limit: U256,
    pub base_fee_per_gas: U256,
    pub next_base_fee: U256,
    pub timestamp: U256,
}

pub async fn start_block_collector(block_ws_url: String) -> Arc<RwLock<BlockInfo>>
{
    let reader = Arc::new(RwLock::new(BlockInfo::default()));
    let reader_clone = reader.clone();

    tokio::spawn(async move {
        loop {
            let provider = Provider::<Ws>::connect(block_ws_url.clone()).await.unwrap();
            let mut stream = provider.subscribe_blocks().await.unwrap();
        
            while let Some(block) = stream.next().await {
                let new_block = BlockInfo {
                    number: block.number.unwrap_or_default().as_u64().into(),
                    gas_limit: block.gas_limit,
                    gas_used: block.gas_used,
                    base_fee_per_gas: block.base_fee_per_gas.unwrap_or_default(),
                    next_base_fee: calculate_next_block_base_fee(
                        block.base_fee_per_gas.unwrap_or_default(),
                        block.gas_used,
                        block.gas_limit),
                    timestamp: block.timestamp,
                };
                *reader_clone.write().await = new_block;
            }
        }
    });

    reader
}


/// Calculate the next block base fee
// based on math provided here: https://ethereum.stackexchange.com/questions/107173/how-is-the-base-fee-per-gas-computed-for-a-new-block
pub fn calculate_next_block_base_fee(base_fee_per_gas: U256, gas_used: U256, gas_limit: U256) -> U256 {
    // Get the block base fee per gas
    let current_base_fee_per_gas = base_fee_per_gas;
    let current_gas_used = gas_used;
    let current_gas_target = gas_limit / 2;

    if current_gas_used == current_gas_target {
        current_base_fee_per_gas
    } else if current_gas_used > current_gas_target {
        let gas_used_delta = current_gas_used - current_gas_target;
        let base_fee_per_gas_delta =
            current_base_fee_per_gas * gas_used_delta / current_gas_target / 8;

        return current_base_fee_per_gas + base_fee_per_gas_delta;
    } else {
        let gas_used_delta = current_gas_target - current_gas_used;
        let base_fee_per_gas_delta =
            current_base_fee_per_gas * gas_used_delta / current_gas_target / 8;

        return current_base_fee_per_gas - base_fee_per_gas_delta;
    }
}