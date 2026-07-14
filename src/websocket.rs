use futures_util::StreamExt;
use tokio::sync::mpsc::Sender;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use std::time::Duration;

/// Connects to the backpack.tf websocket and streams listings to the DB ingestion worker
pub async fn start_listener(tx: Sender<String>) {
    println!("📡 [Websocket] Connecting to backpack.tf firehose...");

    // The official backpack.tf websocket endpoint
    let ws_url = "wss://ws.backpack.tf/events";

    // Infinite auto-recovery loop
    loop {
        match connect_async(ws_url).await {
            Ok((ws_stream, _)) => {
                println!("✅ [Websocket] Successfully connected to backpack.tf!");
                
                // Split the stream to read incoming messages
                let (_, mut read) = ws_stream.split();

                // Process each incoming message as long as the connection is alive
                while let Some(message) = read.next().await {
                    match message {
                        Ok(Message::Text(text)) => {
                            // Send the raw JSON payload to the DB worker thread
                            if let Err(e) = tx.send(text.to_string()).await {
                                eprintln!("❌ [Websocket] Failed to send message to internal channel: {}", e);
                            }
                        }
                        Ok(Message::Close(_)) => {
                            println!("⚠️ [Websocket] Connection closed by server. Initiating reconnect...");
                            break; // Break the inner loop to trigger a reconnect
                        }
                        Err(e) => {
                            eprintln!("❌ [Websocket] Read error: {}. Initiating reconnect...", e);
                            break; // Break the inner loop to trigger a reconnect
                        }
                        _ => {} // Ignore Ping/Pong and Binary frames for now
                    }
                }
            }
            Err(e) => {
                eprintln!("❌ [Websocket] Connection failed: {}. Retrying in 5 seconds...", e);
            }
        }

        // Wait 5 seconds before attempting to reconnect to avoid spamming the server
        tokio::time::sleep(Duration::from_secs(5)).await;
        println!("🔄 [Websocket] Attempting to reconnect...");
    }
}