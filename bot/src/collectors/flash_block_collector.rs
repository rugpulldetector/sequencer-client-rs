use crate::{collectors::feed_clients::RelayClients, types::FlashblocksPayloadV1};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};

pub async fn start_flash_block_collector(flash_block_ws_url: String, chain_id: u64) -> 
    UnboundedReceiver<FlashblocksPayloadV1>
{
    // Create a channel to receive messages from the feed client
    let (sender_fb, receiver_fb) = unbounded_channel();

    // Create a new relay client and start background maintenance
    let relay_client = RelayClients::new(&
        flash_block_ws_url, chain_id, 20, 1, 
        sender_fb.clone())
        .await
        .expect("Failed to create relay client");
    tokio::spawn(RelayClients::start_reader(relay_client));

    receiver_fb
}