use std::sync::Arc;

use ethers::{middleware::SignerMiddleware, providers::Middleware, signers::{LocalWallet, Signer}, types::{transaction::eip2718::TypedTransaction, H160, U256}};
use ethers_flashbots::{FlashbotsMiddleware, Relay};
use ethers_providers::{Http, Provider};
use serde_json::json;
use tokio::sync::mpsc::UnboundedReceiver;
use url::Url;

pub async fn start_titan_executor(
    rpc_client: Arc<Provider<Http>>,
    wallet: LocalWallet,
    chain_id: u64,
    mut tx_receiver: UnboundedReceiver<(TypedTransaction, Vec<H160>, U256)>
) {
    tokio::spawn(async move {
        let titan_client = Relay::new(Url::parse("https://rpc.titanbuilder.xyz").unwrap(), Some(wallet.clone()));
        while let Some((mut tx, target_pools, block_number)) = tx_receiver.recv().await {
            let flashbots_client = SignerMiddleware::new(
                FlashbotsMiddleware::new(
                    rpc_client.clone(),
                    Url::parse("https://relay.flashbots.net").unwrap(),
                    wallet.clone(),
                ),
                wallet.clone(),
            );
    
                let nonce = rpc_client.get_transaction_count(tx.from().unwrap().clone(), None).await.unwrap();
            tx.set_nonce(nonce);
            tx.set_chain_id(chain_id);

            // Send to titan
            let signature = match wallet.clone().sign_transaction(&tx).await {
                Ok(sig) => sig,
                Err(e) => {
                    eprintln!("Failed to sign transaction: {}", e);
                    continue;
                }
            };
            
            let signed_tx = tx.rlp_signed(&signature);
            let params = json!([{
                "txs": vec![signed_tx],
                "blockNumber": block_number,
                "targetPools": target_pools,
            }]).as_array().unwrap().to_vec();

            println!("Titan params: {:?}", params);

            let titan_client_clone = titan_client.clone();
            tokio::spawn(async move {
                match titan_client_clone.request::<_, serde_json::Value>("eth_sendEndOfBlockBundle", params).await {
                    Ok(response) => {
                        println!("Titan: {:?}", response);
                    }
                    Err(e) => {
                        eprintln!("Titan error: {}", e);
                    }
                }
            });

            // Send to flashbots
            tokio::spawn(async move {
                match flashbots_client.send_transaction(tx, None).await {
                    Ok(response) => {
                        println!("{:?}", response);
                        match response.await {
                            Ok(tx) => {
                                println!("Flashbots result: {:?}", tx);
                            }
                            Err(e) => {
                                eprintln!("Flashbots error: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to send Flashbots transaction: {}", e);
                    }
                }
            });
        }
    });
}