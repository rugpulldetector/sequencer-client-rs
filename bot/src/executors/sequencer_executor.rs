use std::sync::Arc;

use ethers::{middleware::SignerMiddleware, providers::Middleware, signers::{LocalWallet, Signer}, types::{transaction::eip2718::TypedTransaction, U256}};
use ethers_providers::{Http, Provider};
use tokio::sync::mpsc::UnboundedReceiver;

pub async fn start_sequencer_executor(
    sequencer_url: String,
    wallet: LocalWallet,
    chain_id: u64,
    mut starting_nonce: U256,
    mut tx_receiver: UnboundedReceiver<TypedTransaction>
) {
    let sequencer_client = Arc::new(Provider::<Http>::try_from(sequencer_url).unwrap());
    let sequencer_client = SignerMiddleware::new(Arc::new(sequencer_client), wallet.clone());

    tokio::spawn(async move {
        while let Some(mut tx) = tx_receiver.recv().await {
            tx.set_nonce(starting_nonce);
            tx.set_chain_id(chain_id);
    
            let signature = wallet.sign_transaction(&tx).await.unwrap();
            let signed_tx = tx.rlp_signed(&signature);
            let sequencer_client_clone = sequencer_client.clone();
        
            tokio::spawn(async move {
                match sequencer_client_clone.send_raw_transaction(signed_tx).await {
                    Ok(pending_tx) => {
                        println!("[MS] Tx sent: {:?}", tx);
                        match pending_tx.await {
                            Ok(tx) => println!("[MS] Tx mined: {:?}", tx),
                            Err(e) => println!("[MS] Tx error: {}", e),
                        }
                    }
                    Err(e) => println!("[MS] Send error: {}", e),
                }
            });
        
            starting_nonce = starting_nonce + U256::from(1);            
        }
    });
}