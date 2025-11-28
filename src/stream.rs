use alpaca_markets::{
    AlpacaConfig,
    clients::trading_stream::TradingStreamClient,
    wss::trading::{StreamData, TradeUpdate, TradingWebSocketMessage},
};
use std::thread;
use tokio::sync::mpsc;

/// Message types that can be sent from the WebSocket to the UI
#[derive(Clone, Debug)]
pub enum StreamUpdate {
    Connected,
    Disconnected,
    TradeUpdate(OrderUpdate),
    AccountUpdate(AccountInfo),
    Error(String),
}

/// Order update information from trade events
#[derive(Clone, Debug)]
pub struct OrderUpdate {
    pub id: String,
    pub symbol: String,
    pub side: String,
    pub qty: String,
    pub order_type: String,
    pub limit_price: Option<String>,
    pub status: String,
    pub created_at: String,
    pub event: String,
}

/// Account information from account updates
#[derive(Clone, Debug)]
pub struct AccountInfo {
    pub buying_power: String,
    pub cash: String,
    pub portfolio_value: String,
}

/// WebSocket stream manager
pub struct StreamManager {
    sender: mpsc::UnboundedSender<StreamUpdate>,
    receiver: mpsc::UnboundedReceiver<StreamUpdate>,
}

impl StreamManager {
    /// Create a new stream manager
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        Self { sender, receiver }
    }

    /// Get a sender handle for spawning the WebSocket task
    pub fn get_sender(&self) -> mpsc::UnboundedSender<StreamUpdate> {
        self.sender.clone()
    }

    /// Take the receiver (can only be done once)
    pub fn take_receiver(&mut self) -> Option<mpsc::UnboundedReceiver<StreamUpdate>> {
        // We need to return a new receiver, but we can't clone mpsc receivers
        // So we'll create a new channel pair and swap
        let (new_sender, new_receiver) = mpsc::unbounded_channel();
        let old_receiver = std::mem::replace(&mut self.receiver, new_receiver);
        self.sender = new_sender;
        Some(old_receiver)
    }

    /// Start the WebSocket connection in a background task
    pub fn start_stream(sender: mpsc::UnboundedSender<StreamUpdate>) -> thread::JoinHandle<()> {
        thread::spawn(move || {
            // Create a Tokio runtime for this thread
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async move {
                println!("🚀 Starting Alpaca Trading WebSocket stream...");

                // Create configuration
                let config = match AlpacaConfig::from_env() {
                    Ok(config) => {
                        println!("✅ Configuration loaded from environment variables");
                        config
                    }
                    Err(_) => {
                        println!("⚠️  Environment variables not found. Using demo configuration.");
                        println!(
                            "   To use real data, set APCA_API_KEY_ID and APCA_API_SECRET_KEY"
                        );

                        AlpacaConfig::new(
                            "DEMO_KEY".to_string(),
                            "DEMO_SECRET".to_string(),
                            true, // Use paper trading
                        )
                    }
                };

                // Create trading stream client
                let mut client = TradingStreamClient::new(config);

                println!("🔌 Connecting to Alpaca Trading WebSocket...");

                match client.connect().await {
                    Ok(_) => {
                        println!("✅ Connected to trading stream!");
                        let _ = sender.send(StreamUpdate::Connected);
                    }
                    Err(e) => {
                        eprintln!("❌ Connection failed: {}", e);
                        let _ =
                            sender.send(StreamUpdate::Error(format!("Connection failed: {}", e)));
                        let _ = sender.send(StreamUpdate::Disconnected);
                        return;
                    }
                }

                // Process messages
                loop {
                    match client.next_message().await {
                        Ok(Some(message)) => {
                            if let Some(update) = process_message(message) {
                                if sender.send(update).is_err() {
                                    println!("❌ Failed to send update to UI (channel closed)");
                                    break;
                                }
                            }
                        }
                        Ok(None) => {
                            // None can mean:
                            // 1. Control frame (Ping/Pong) - already logged by library
                            // 2. Parse error - already logged by library with raw message
                            // Just continue processing, no additional warning needed
                            continue;
                        }
                        Err(e) => {
                            // Check if it's a serialization error (unsupported message type)
                            let error_str = e.to_string();
                            if error_str.contains("Serialization error")
                                || error_str.contains("Unsupported message type")
                            {
                                println!("⚠️  Skipping unsupported message type: {}", error_str);
                                // Continue processing, don't disconnect
                                continue;
                            }

                            eprintln!("❌ Error receiving message: {}", e);
                            let _ =
                                sender.send(StreamUpdate::Error(format!("Stream error: {}", e)));

                            // Try to reconnect after a delay
                            println!("🔄 Attempting to reconnect in 5 seconds...");
                            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

                            match client.connect().await {
                                Ok(_) => {
                                    println!("✅ Reconnected successfully!");
                                    let _ = sender.send(StreamUpdate::Connected);
                                }
                                Err(e) => {
                                    eprintln!("❌ Reconnection failed: {}", e);
                                    let _ = sender.send(StreamUpdate::Disconnected);
                                    break;
                                }
                            }
                        }
                    }
                }

                println!("🛑 WebSocket stream task ended");
            })
        })
    }
}

/// Process incoming WebSocket messages and convert to StreamUpdate
fn process_message(message: TradingWebSocketMessage) -> Option<StreamUpdate> {
    match message {
        TradingWebSocketMessage::StreamMessage(stream_msg) => match stream_msg.data {
            StreamData::TradeUpdate(trade_update) => {
                println!(
                    "🔄 Trade Update: {} - Order {} ({}) is now {}",
                    trade_update.event,
                    trade_update.order.id,
                    trade_update.order.symbol,
                    trade_update.order.status
                );

                Some(StreamUpdate::TradeUpdate(convert_trade_update(
                    trade_update,
                )))
            }
            StreamData::AccountUpdate(account_update) => {
                println!(
                    "💰 Account Update: Buying Power: ${}, Cash: ${}",
                    account_update.buying_power, account_update.cash
                );

                Some(StreamUpdate::AccountUpdate(AccountInfo {
                    buying_power: account_update.buying_power,
                    cash: account_update.cash,
                    portfolio_value: account_update.total_portfolio_value,
                }))
            }
            StreamData::Listening(listening) => {
                println!("👂 Subscribed to: {:?}", listening.streams);
                None
            }
        },
        TradingWebSocketMessage::Connected(connected) => {
            println!("🔗 Connection: {}", connected.msg);
            None
        }
        TradingWebSocketMessage::Authorization(auth) => {
            println!("🔐 Auth: {} -> {}", auth.action, auth.status);
            None
        }
        TradingWebSocketMessage::Error(error) => {
            eprintln!("❌ Error [{}]: {}", error.code, error.msg);
            Some(StreamUpdate::Error(format!(
                "[{}] {}",
                error.code, error.msg
            )))
        }
        TradingWebSocketMessage::Unknown(data) => {
            println!("❓ Unknown message: {}", data);
            None
        }
    }
}

/// Convert TradeUpdate to OrderUpdate
fn convert_trade_update(trade: TradeUpdate) -> OrderUpdate {
    OrderUpdate {
        id: trade.order.id.clone(),
        symbol: trade.order.symbol.clone(),
        side: trade.order.side.clone(),
        qty: trade
            .order
            .qty
            .unwrap_or_else(|| trade.order.filled_qty.clone()),
        order_type: trade.order.order_type.clone(),
        limit_price: trade.order.limit_price.clone(),
        status: trade.order.status.clone(),
        created_at: trade.order.created_at.to_rfc3339(),
        event: trade.event.to_string(),
    }
}
