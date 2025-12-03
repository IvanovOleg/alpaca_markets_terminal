// Account module for managing trading account, positions, and orders

use alpaca_markets::models::{OrderRequest, OrderSide, OrderTimeInForce, OrderType};
use alpaca_markets::{AlpacaConfig, TradingClient};

/// Position information
#[derive(Clone)]
pub struct Position {
    pub symbol: String,
    pub qty: String,
    pub avg_entry_price: String,
    pub current_price: String,
    pub market_value: String,
    pub unrealized_pl: String,
    pub unrealized_plpc: String,
}

/// Order information
#[derive(Clone)]
pub struct Order {
    pub id: String,
    pub symbol: String,
    pub side: String,
    pub qty: String,
    pub order_type: String,
    pub limit_price: Option<String>,
    pub status: String,
    pub created_at: String,
}

/// Footer tab selection
#[derive(Clone, PartialEq)]
pub enum FooterTab {
    Account,
    Positions,
    Orders,
}

/// Account state containing all account-related fields
pub struct Account {
    // Account information
    pub account_number: Option<String>,
    pub account_status: Option<String>,
    pub buying_power: Option<f64>,
    pub cash: Option<f64>,
    pub portfolio_value: Option<f64>,
    pub equity: Option<f64>,
    pub account_loading: bool,

    // Positions information
    pub positions: Vec<Position>,
    pub positions_loading: bool,

    // Orders information
    pub orders: Vec<Order>,
    pub orders_loading: bool,
    pub active_footer_tab: FooterTab,

    // Order form fields
    pub order_side: OrderSide,
    pub order_type: OrderType,
    pub order_quantity: String,
    pub order_limit_price: String,
    pub order_time_in_force: OrderTimeInForce,
    pub order_submitting: bool,
    pub order_message: Option<String>,

    // Input focus tracking
    pub quantity_focused: bool,
    pub price_focused: bool,
}

impl Account {
    pub fn new() -> Self {
        Self {
            account_number: None,
            account_status: None,
            buying_power: None,
            cash: None,
            portfolio_value: None,
            equity: None,
            account_loading: true,
            positions: Vec::new(),
            positions_loading: true,
            orders: Vec::new(),
            orders_loading: true,
            active_footer_tab: FooterTab::Account,
            order_side: OrderSide::Buy,
            order_type: OrderType::Market,
            order_quantity: String::new(),
            order_limit_price: String::new(),
            order_time_in_force: OrderTimeInForce::Day,
            order_submitting: false,
            order_message: None,
            quantity_focused: false,
            price_focused: false,
        }
    }

    /// Update account information from stream
    pub fn update_from_stream(&mut self, account_info: crate::stream::AccountInfo) {
        if let Ok(buying_power) = account_info.buying_power.parse::<f64>() {
            self.buying_power = Some(buying_power);
        }

        if let Ok(cash) = account_info.cash.parse::<f64>() {
            self.cash = Some(cash);
        }

        if let Ok(portfolio_value) = account_info.portfolio_value.parse::<f64>() {
            self.portfolio_value = Some(portfolio_value);
        }

        println!("✓ Account updated from stream");
    }

    /// Update order from stream
    pub fn update_order_from_stream(&mut self, order_update: crate::stream::OrderUpdate) {
        // Check if this is a terminal state - remove from list immediately
        let is_terminal_state = matches!(
            order_update.status.as_str(),
            "filled" | "canceled" | "expired" | "rejected"
        );

        if is_terminal_state {
            // Remove the order from the list
            if let Some(pos) = self.orders.iter().position(|o| o.id == order_update.id) {
                self.orders.remove(pos);
                println!(
                    "🗑️  Removed {} order {} from list",
                    order_update.status, order_update.id
                );
            } else {
                println!(
                    "ℹ️  Order {} is {} but not found in list",
                    order_update.id, order_update.status
                );
            }
            return;
        }

        // Find and update existing order, or add new one
        if let Some(existing_order) = self.orders.iter_mut().find(|o| o.id == order_update.id) {
            // Update existing order
            existing_order.symbol = order_update.symbol.clone();
            existing_order.side = order_update.side.clone();
            existing_order.qty = order_update.qty.clone();
            existing_order.order_type = order_update.order_type.clone();
            existing_order.limit_price = order_update.limit_price.clone();
            existing_order.status = order_update.status.clone();
            existing_order.created_at = order_update.created_at.clone();

            println!(
                "✓ Updated order {} - Status: {}",
                existing_order.id, existing_order.status
            );
        } else {
            // Add new order (only if not terminal state)
            let new_order = Order {
                id: order_update.id.clone(),
                symbol: order_update.symbol.clone(),
                side: order_update.side.clone(),
                qty: order_update.qty.clone(),
                order_type: order_update.order_type.clone(),
                limit_price: order_update.limit_price.clone(),
                status: order_update.status.clone(),
                created_at: order_update.created_at.clone(),
            };

            println!("✓ Added new order {}", new_order.id);
            self.orders.push(new_order);
        }
    }
}

// Synchronous API functions (run in background threads)

/// Fetch account information
pub fn fetch_account_sync() -> Result<(String, String, f64, f64, f64, f64), String> {
    let rt = tokio::runtime::Runtime::new().map_err(|e| format!("Runtime error: {:?}", e))?;

    rt.block_on(async {
        let config = match AlpacaConfig::from_env() {
            Ok(config) => config,
            Err(e) => {
                return Err(format!(
                    "Error loading config: {:?}. Please set APCA_API_KEY_ID and APCA_API_SECRET_KEY environment variables.",
                    e
                ));
            }
        };

        let client = TradingClient::new(config);

        let result = client.get_account().await;

        match result {
            Ok(account) => {
                // Parse string values to f64
                let buying_power = account.buying_power.parse::<f64>().unwrap_or(0.0);
                let cash = account.cash.parse::<f64>().unwrap_or(0.0);
                let portfolio_value = account.portfolio_value.parse::<f64>().unwrap_or(0.0);
                let equity = account.equity.parse::<f64>().unwrap_or(0.0);

                Ok((
                    account.account_number,
                    format!("{:?}", account.status),
                    buying_power,
                    cash,
                    portfolio_value,
                    equity,
                ))
            },
            Err(e) => Err(format!("Error fetching account: {:?}", e)),
        }
    })
}

/// Fetch positions
pub fn fetch_positions_sync() -> Result<Vec<Position>, String> {
    let rt = tokio::runtime::Runtime::new().map_err(|e| format!("Runtime error: {:?}", e))?;

    rt.block_on(async {
        let config = match AlpacaConfig::from_env() {
            Ok(config) => config,
            Err(e) => {
                return Err(format!(
                    "Error loading config: {:?}. Please set APCA_API_KEY_ID and APCA_API_SECRET_KEY environment variables.",
                    e
                ));
            }
        };

        let client = TradingClient::new(config);

        let result = client.get_positions().await;

        match result {
            Ok(positions) => {
                let mapped_positions = positions
                    .into_iter()
                    .map(|p| Position {
                        symbol: p.symbol,
                        qty: p.qty,
                        avg_entry_price: p.avg_entry_price,
                        current_price: p.current_price,
                        market_value: p.market_value,
                        unrealized_pl: p.unrealized_pl,
                        unrealized_plpc: p.unrealized_plpc,
                    })
                    .collect();
                Ok(mapped_positions)
            }
            Err(e) => Err(format!("Error fetching positions: {:?}", e)),
        }
    })
}

/// Fetch orders
pub fn fetch_orders_sync() -> Result<Vec<Order>, String> {
    let rt = tokio::runtime::Runtime::new().map_err(|e| format!("Runtime error: {:?}", e))?;

    rt.block_on(async {
        let config = match AlpacaConfig::from_env() {
            Ok(config) => config,
            Err(e) => {
                return Err(format!(
                    "Error loading config: {:?}. Please set APCA_API_KEY_ID and APCA_API_SECRET_KEY environment variables.",
                    e
                ));
            }
        };

        let client = TradingClient::new(config);

        // Get open orders (status="open")
        let result = client.get_orders(Some("open"), Some(50)).await;

        match result {
            Ok(orders) => {
                let mapped_orders = orders
                    .into_iter()
                    .map(|o| Order {
                        id: o.id,
                        symbol: o.symbol,
                        side: format!("{:?}", o.side),
                        qty: o.qty.unwrap_or("0".to_string()),
                        order_type: format!("{:?}", o.order_type),
                        limit_price: o.limit_price,
                        status: format!("{:?}", o.status),
                        created_at: o.created_at.format("%Y-%m-%d %H:%M").to_string(),
                    })
                    .collect();
                Ok(mapped_orders)
            }
            Err(e) => Err(format!("Error fetching orders: {:?}", e)),
        }
    })
}

/// Submit an order
pub fn submit_order_sync(
    symbol: String,
    side: OrderSide,
    order_type: OrderType,
    qty: f64,
    limit_price: Option<f64>,
    time_in_force: OrderTimeInForce,
) -> Result<String, String> {
    let rt = tokio::runtime::Runtime::new().map_err(|e| format!("Runtime error: {:?}", e))?;

    rt.block_on(async {
        let config = match AlpacaConfig::from_env() {
            Ok(config) => config,
            Err(e) => {
                return Err(format!(
                    "Error loading config: {:?}. Please set APCA_API_KEY_ID and APCA_API_SECRET_KEY environment variables.",
                    e
                ));
            }
        };

        let client = TradingClient::new(config);

        let order_request = OrderRequest {
            symbol: symbol.clone(),
            qty: Some(qty.to_string()),
            notional: None,
            side,
            order_type,
            time_in_force,
            limit_price: limit_price.map(|p| p.to_string()),
            stop_price: None,
            extended_hours: Some(false),
            client_order_id: None,
            order_class: None,
            take_profit: None,
            stop_loss: None,
            trail_price: None,
            trail_percent: None,
        };

        let result = client.submit_order(order_request).await;

        match result {
            Ok(order) => Ok(order.id),
            Err(e) => Err(format!("Failed to submit order: {:?}", e)),
        }
    })
}

/// Cancel an order
pub fn cancel_order_sync(order_id: String) -> Result<(), String> {
    let rt = tokio::runtime::Runtime::new().map_err(|e| format!("Runtime error: {:?}", e))?;

    rt.block_on(async {
        let config = match AlpacaConfig::from_env() {
            Ok(config) => config,
            Err(e) => {
                return Err(format!(
                    "Error loading config: {:?}. Please set APCA_API_KEY_ID and APCA_API_SECRET_KEY environment variables.",
                    e
                ));
            }
        };

        let client = TradingClient::new(config);

        let result = client.cancel_order(&order_id).await;

        match result {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("Failed to cancel order: {:?}", e)),
        }
    })
}

/// Close a position
pub fn close_position_sync(symbol: String) -> Result<(), String> {
    let rt = tokio::runtime::Runtime::new().map_err(|e| format!("Runtime error: {:?}", e))?;

    rt.block_on(async {
        let config = match AlpacaConfig::from_env() {
            Ok(config) => config,
            Err(e) => {
                return Err(format!(
                    "Error loading config: {:?}. Please set APCA_API_KEY_ID and APCA_API_SECRET_KEY environment variables.",
                    e
                ));
            }
        };

        let client = TradingClient::new(config);

        let result = client.close_position(&symbol, None, None).await;

        match result {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("Failed to close position: {:?}", e)),
        }
    })
}
