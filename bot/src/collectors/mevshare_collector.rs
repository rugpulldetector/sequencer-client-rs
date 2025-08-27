
use mev_share::sse::{Event, EventClient};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};
use tokio_stream::StreamExt;

pub async fn start_mevshare_collector(mevshare_sse_url: String) -> 
    UnboundedReceiver<Event>
{
    // Create a channel to receive messages from the feed client
    let (sender_fb, receiver_fb) = unbounded_channel();

    tokio::spawn(async move {
        loop {
            let client = EventClient::default();
            let mut stream = client.events(&mevshare_sse_url).await.unwrap();
            
            while let Some(event) = stream.next().await {
                let Ok(event) = event else {
                    continue;
                };
                sender_fb.send(event);
            }
        }
    });
    
    receiver_fb

}