use std::io::Cursor;

use crate::errors::{ConnectionUpdate, RelayError};
use crate::types::FlashblocksPayloadV1;
use tokio::sync::mpsc::UnboundedSender;
use ethers::providers::StreamExt;
use log::*;
use tokio::net::TcpStream;
use tokio::task::JoinHandle;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};
use tungstenite::Message;
use url::Url;

/// Sequencer Feed Client
pub struct RelayClient {
    // Socket connection to read from
    connection: WebSocketStream<MaybeTlsStream<TcpStream>>,
    // For sending errors / disconnects
    connection_update: UnboundedSender<ConnectionUpdate>,
    // Sends Transactions
    sender: UnboundedSender<FlashblocksPayloadV1>,
    // Relay ID
    id: u32,
}

impl RelayClient {
    // Does not start the reader, only makes the websocket connection
    pub async fn new(
        url: Url,
        id: u32,
        sender: UnboundedSender<FlashblocksPayloadV1>,
        connection_update: UnboundedSender<ConnectionUpdate>,
    ) -> Result<Self, RelayError> {
        info!("Adding client | Client Id: {}", id);

        let key = tungstenite::handshake::client::generate_key();
        let host = url
            .host_str()
            .ok_or(RelayError::InvalidUrl)?;

        let req = tungstenite::handshake::client::Request::builder()
            .method("GET")
            .uri(url.as_str())
            .header("Host", host)
            .header("Connection", "Upgrade")
            .header("Upgrade", "websocket")
            .header("Sec-WebSocket-Version", "13")
            .header("Sec-WebSocket-Key", key)
            .body(())?;

        let (socket, resp) = connect_async(req).await?;


        Ok(Self {
            connection: socket,
            connection_update,
            sender,
            id,
        })
    }

    // Start the reader
    pub fn spawn(self) -> JoinHandle<()> {
        println!("Sequencer feed reader started | Client Id: {}", self.id);

        tokio::spawn(async move {
            match self.run().await {
                Ok(_) => (),
                Err(e) => error!("{}", e),
            }
        })
    }

    pub async fn run(mut self) -> Result<(), RelayError> {
        while let Some(msg) = self.connection.next().await {
            match msg {
                Ok(Message::Binary(bytes)) => {
                    // Decode binary message to string first
                    let mut reader = Cursor::new(bytes);
                    let mut decompressed = Vec::new();
                    brotli_decompressor::BrotliDecompress(&mut reader, &mut decompressed)?;
                
                    let text = String::from_utf8(decompressed).unwrap();
                    let flashblock: FlashblocksPayloadV1 = serde_json::from_str(&text).unwrap();
                    
                    if self.sender.send(flashblock).is_err() {
                        break; // we gracefully exit
                    }

                }
                Ok(Message::Close(_)) => break,

                    // println!("message: {:?}", message);
                    // let decoded_root: Root = match serde_json::from_slice(&message.into_data()) {
                    //     Ok(d) => d,
                    //     Err(_) => continue,
                    // };

                Err(e) => {
                    let _ = self.connection_update
                        .send(ConnectionUpdate::StoppedSendingFrames(self.id));
                    error!("Connection closed with error: {}", e);
                    break;
                },
                _ => {}
            }
        }

        Ok(())
    }
}
