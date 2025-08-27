//! Collectors are responsible for collecting data from external sources and
//! turning them into internal events. For example, a collector might listen to
//! a stream of new blocks, and turn them into a stream of `NewBlock` events.

/// This collector listens to a stream of new blocks.
pub mod block_collector;

/// This collector listens to a stream of sequencer feed
pub mod binance_collector;

pub mod mevshare_collector;

/// This collector listens to a stream of flash block
pub mod flash_block_collector;
pub mod feed_client;
pub mod feed_clients;