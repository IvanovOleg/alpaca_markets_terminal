use alpaca_markets::{
    AlpacaConfig, Bar, MarketDataClient, TradingClient,
    models::{OrderSide, OrderTimeInForce, OrderType},
};
use chrono::{Duration, Utc};
use gpui::{
    App, Application, Context, ElementId, FocusHandle, FontWeight, IntoElement, Render, Window,
    WindowOptions, actions, div, prelude::*, px, rgb,
};

mod stream;
use stream::{StreamManager, StreamUpdate};
use tokio::sync::mpsc;

actions!(app, [Quit, RefreshData]);

#[derive(Clone)]
struct Position {
    symbol: String,
    qty: String,
    avg_entry_price: String,
    current_price: String,
    market_value: String,
    unrealized_pl: String,
    unrealized_plpc: String,
}

#[derive(Clone)]
struct Order {
    id: String,
    symbol: String,
    side: String,
    qty: String,
    order_type: String,
    limit_price: Option<String>,
    status: String,
    created_at: String,
}

#[derive(Clone, PartialEq)]
enum FooterTab {
    Account,
    Positions,
    Orders,
}

struct BarChart {
    symbol: String,
    symbol_input: String,
    timeframe: String,
    bars: Vec<Bar>,
    loading: bool,
    error: Option<String>,
    input_focused: bool,
    focus_handle: FocusHandle,
    // Account information
    account_number: Option<String>,
    account_status: Option<String>,
    buying_power: Option<f64>,
    cash: Option<f64>,
    portfolio_value: Option<f64>,
    equity: Option<f64>,
    account_loading: bool,
    // Positions information
    positions: Vec<Position>,
    positions_loading: bool,
    // Orders information
    orders: Vec<Order>,
    orders_loading: bool,
    active_footer_tab: FooterTab,
    // Order form fields
    order_side: OrderSide,
    order_type: OrderType,
    order_quantity: String,
    order_limit_price: String,
    order_time_in_force: OrderTimeInForce,
    order_submitting: bool,
    order_message: Option<String>,
    // Input focus tracking
    quantity_focused: bool,
    price_focused: bool,
    // WebSocket stream
    stream_connected: bool,
    stream_status: String,
    // Market data stream
    market_data_connected: bool,
    last_bar_time: Option<String>,
}

impl BarChart {
    fn new(cx: &mut Context<Self>) -> Self {
        let mut chart = Self {
            symbol: "AAPL".to_string(),
            symbol_input: "AAPL".to_string(),
            timeframe: "1Day".to_string(),
            bars: Vec::new(),
            loading: true,
            error: None,
            input_focused: false,
            focus_handle: cx.focus_handle(),
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
            order_quantity: "".to_string(),
            order_limit_price: "".to_string(),
            order_time_in_force: OrderTimeInForce::Day,
            order_submitting: false,
            order_message: None,
            quantity_focused: false,
            price_focused: false,
            stream_connected: false,
            stream_status: "Disconnected".to_string(),
            market_data_connected: false,
            last_bar_time: None,
        };

        // Fetch data on startup
        chart.fetch_bars(cx);
        chart.fetch_account(cx);
        chart.fetch_positions(cx);
        chart.start_websocket_stream(cx);
        chart.start_market_data_stream(cx);
        chart.fetch_orders(cx);
        chart
    }

    fn handle_input(&mut self, text: &str, cx: &mut Context<Self>) {
        if !self.input_focused {
            return;
        }
        self.symbol_input.push_str(text);
        cx.notify();
    }

    fn handle_backspace(&mut self, cx: &mut Context<Self>) {
        if !self.input_focused {
            return;
        }
        self.symbol_input.pop();
        cx.notify();
    }

    fn submit_symbol(&mut self, cx: &mut Context<Self>) {
        if !self.symbol_input.is_empty() {
            self.symbol = self.symbol_input.clone().to_uppercase();
            self.input_focused = false;
            self.fetch_bars(cx);
        }
    }

    fn fetch_account(&mut self, cx: &mut Context<Self>) {
        self.account_loading = true;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { fetch_account_sync() })
                .await;

            let _ = this.update(cx, |chart, cx| {
                match result {
                    Ok(account_data) => {
                        chart.account_number = Some(account_data.0);
                        chart.account_status = Some(account_data.1);
                        chart.buying_power = Some(account_data.2);
                        chart.cash = Some(account_data.3);
                        chart.portfolio_value = Some(account_data.4);
                        chart.equity = Some(account_data.5);
                        println!("✓ Successfully loaded account information");
                    }
                    Err(error) => {
                        eprintln!("✗ Error fetching account: {}", error);
                        chart.account_status = Some("Error".to_string());
                    }
                }
                chart.account_loading = false;
                cx.notify();
            });
        })
        .detach();
    }

    fn fetch_positions(&mut self, cx: &mut Context<Self>) {
        self.positions_loading = true;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { fetch_positions_sync() })
                .await;

            let _ = this.update(cx, |chart, cx| {
                match result {
                    Ok(positions) => {
                        chart.positions = positions;
                        println!("✓ Successfully loaded {} positions", chart.positions.len());
                    }
                    Err(error) => {
                        eprintln!("✗ Error fetching positions: {}", error);
                        chart.positions.clear();
                    }
                }
                chart.positions_loading = false;
                cx.notify();
            });
        })
        .detach();
    }

    fn fetch_orders(&mut self, cx: &mut Context<Self>) {
        self.orders_loading = true;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { fetch_orders_sync() })
                .await;

            let _ = this.update(cx, |chart, cx| {
                match result {
                    Ok(mut orders) => {
                        // Filter out terminal state orders (filled, canceled, expired, rejected)
                        orders.retain(|order| {
                            !matches!(
                                order.status.as_str(),
                                "filled" | "canceled" | "expired" | "rejected"
                            )
                        });
                        chart.orders = orders;
                        println!("✓ Successfully loaded {} active orders", chart.orders.len());
                    }
                    Err(error) => {
                        eprintln!("✗ Error fetching orders: {}", error);
                        chart.orders.clear();
                    }
                }
                chart.orders_loading = false;
                cx.notify();
            });
        })
        .detach();
    }

    fn cancel_order(&mut self, order_id: String, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { cancel_order_sync(order_id) })
                .await;

            let _ = this.update(cx, |_chart, cx| {
                match result {
                    Ok(_) => {
                        println!("✓ Order canceled successfully");
                        // WebSocket will handle the order update automatically
                        cx.notify();
                    }
                    Err(error) => {
                        eprintln!("✗ Error canceling order: {}", error);
                    }
                }
            });
        })
        .detach();
    }

    fn close_position(&mut self, symbol: String, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { close_position_sync(symbol) })
                .await;

            let _ = this.update(cx, |chart, cx| {
                match result {
                    Ok(_) => {
                        println!("✓ Position closed successfully");
                        // Refresh positions list (WebSocket handles order updates)
                        chart.fetch_positions(cx);
                    }
                    Err(error) => {
                        eprintln!("✗ Error closing position: {}", error);
                    }
                }
            });
        })
        .detach();
    }

    fn submit_order(&mut self, cx: &mut Context<Self>) {
        // Validate inputs
        if self.order_quantity.trim().is_empty() {
            self.order_message = Some("Error: Quantity cannot be empty".to_string());
            cx.notify();
            return;
        }

        let qty = match self.order_quantity.parse::<f64>() {
            Ok(q) if q > 0.0 => q,
            _ => {
                self.order_message = Some("Error: Invalid quantity".to_string());
                cx.notify();
                return;
            }
        };

        if matches!(self.order_type, OrderType::Limit) && self.order_limit_price.trim().is_empty() {
            self.order_message = Some("Error: Limit price required for limit orders".to_string());
            cx.notify();
            return;
        }

        let limit_price = if matches!(self.order_type, OrderType::Limit) {
            match self.order_limit_price.parse::<f64>() {
                Ok(p) if p > 0.0 => Some(p),
                _ => {
                    self.order_message = Some("Error: Invalid limit price".to_string());
                    cx.notify();
                    return;
                }
            }
        } else {
            None
        };

        self.order_submitting = true;
        self.order_message = None;
        cx.notify();

        let symbol = self.symbol.clone();
        let side = match self.order_side {
            OrderSide::Buy => OrderSide::Buy,
            OrderSide::Sell => OrderSide::Sell,
        };
        let order_type = match self.order_type {
            OrderType::Market => OrderType::Market,
            OrderType::Limit => OrderType::Limit,
            _ => OrderType::Market,
        };
        let time_in_force = match self.order_time_in_force {
            OrderTimeInForce::Day => OrderTimeInForce::Day,
            OrderTimeInForce::Gtc => OrderTimeInForce::Gtc,
            _ => OrderTimeInForce::Day,
        };

        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    submit_order_sync(symbol, side, order_type, qty, limit_price, time_in_force)
                })
                .await;

            let _ = this.update(cx, |chart, cx| {
                match result {
                    Ok(order_id) => {
                        chart.order_message =
                            Some(format!("✓ Order submitted successfully! ID: {}", order_id));
                        chart.order_quantity = "".to_string();
                        chart.order_limit_price = "".to_string();
                        // WebSocket will handle the order update automatically
                    }
                    Err(error) => {
                        chart.order_message = Some(format!("✗ Error: {}", error));
                    }
                }
                chart.order_submitting = false;
                cx.notify();
            });
        })
        .detach();
    }

    fn start_websocket_stream(&mut self, cx: &mut Context<Self>) {
        println!("🚀 Starting WebSocket stream connection...");

        self.stream_status = "Connecting...".to_string();
        cx.notify();

        // Create a channel for receiving updates from the WebSocket
        let (sender, mut receiver) = mpsc::unbounded_channel::<StreamUpdate>();

        // Start the WebSocket stream in a background task
        StreamManager::start_stream(sender);

        // Spawn a task to listen for updates and apply them to the UI
        cx.spawn(async move |this, cx| {
            while let Some(update) = receiver.recv().await {
                let _ = this.update(cx, |chart, cx| {
                    chart.handle_stream_update(update, cx);
                });
            }
        })
        .detach();
    }

    fn handle_stream_update(&mut self, update: StreamUpdate, cx: &mut Context<Self>) {
        match update {
            StreamUpdate::Connected => {
                println!("✅ WebSocket connected!");
                self.stream_connected = true;
                self.stream_status = "Connected".to_string();
                cx.notify();
            }
            StreamUpdate::Disconnected => {
                println!("❌ WebSocket disconnected");
                self.stream_connected = false;
                self.stream_status = "Disconnected".to_string();
                cx.notify();
            }
            StreamUpdate::TradeUpdate(order_update) => {
                println!("📦 Received order update for: {}", order_update.symbol);
                self.update_order_from_stream(order_update);
                cx.notify();
            }
            StreamUpdate::AccountUpdate(account_info) => {
                println!("💰 Received account update");
                self.update_account_from_stream(account_info);
                cx.notify();
            }
            StreamUpdate::Error(error) => {
                eprintln!("❌ Stream error: {}", error);
                self.stream_status = format!("Error: {}", error);
                cx.notify();
            }
            StreamUpdate::MarketDataConnected => {
                println!("✅ Market Data WebSocket connected!");
                self.market_data_connected = true;
                cx.notify();
            }
            StreamUpdate::MarketDataDisconnected => {
                println!("❌ Market Data WebSocket disconnected");
                self.market_data_connected = false;
                cx.notify();
            }
            StreamUpdate::BarUpdate(bar_update) => {
                println!("📊 Received bar update for: {}", bar_update.symbol);
                self.update_bars_from_stream(bar_update, cx);
                cx.notify();
            }
        }
    }

    fn update_order_from_stream(&mut self, order_update: stream::OrderUpdate) {
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

    fn update_account_from_stream(&mut self, account_info: stream::AccountInfo) {
        // Parse and update account information
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

    fn start_market_data_stream(&mut self, cx: &mut Context<Self>) {
        println!("🚀 Starting Market Data WebSocket stream connection...");

        // Create a channel for receiving updates from the WebSocket
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel::<stream::StreamUpdate>();

        // Get the current symbol to subscribe to
        let symbol = self.symbol.clone();

        // Start the market data WebSocket stream in a background task
        stream::MarketDataStreamManager::start_stream(sender, vec![symbol]);

        // Spawn a task to listen for updates and apply them to the UI
        cx.spawn(async move |this, cx| {
            while let Some(update) = receiver.recv().await {
                let _ = this.update(cx, |chart, cx| {
                    chart.handle_stream_update(update, cx);
                });
            }
        })
        .detach();
    }

    fn update_bars_from_stream(&mut self, bar_update: stream::BarUpdate, cx: &mut Context<Self>) {
        // Only update if the bar is for the current symbol
        if bar_update.symbol != self.symbol {
            return;
        }

        // Parse the bar data and convert to Bar struct
        // Note: We need to convert from the stream BarUpdate to alpaca_markets::Bar
        // For now, we'll just update the last_bar_time to show we're receiving data
        self.last_bar_time = Some(bar_update.timestamp.clone());

        println!(
            "✓ Bar updated for {} - O:{} H:{} L:{} C:{} V:{} @ {}",
            bar_update.symbol,
            bar_update.open,
            bar_update.high,
            bar_update.low,
            bar_update.close,
            bar_update.volume,
            bar_update.timestamp
        );

        // Optionally: Add the new bar to the bars list
        // This would require converting BarUpdate to Bar, which depends on
        // the Bar struct definition in alpaca_markets
        // For real-time updates, you might want to:
        // 1. Append the bar to self.bars if it's a new bar
        // 2. Update the last bar if it's an update to the current bar
        // 3. Trigger a chart refresh

        // For now, just notify to update the UI
        cx.notify();
    }

    fn fetch_bars(&mut self, cx: &mut Context<Self>) {
        self.loading = true;
        self.error = None;
        self.bars.clear();
        cx.notify();

        let symbol = self.symbol.clone();
        let timeframe = self.timeframe.clone();

        // Modern GPUI async pattern with AsyncApp::update()
        cx.spawn(async move |this, cx| {
            // Run the blocking API call in a background thread
            let result = cx
                .background_executor()
                .spawn(async move { fetch_bars_sync(&symbol, &timeframe) })
                .await;

            // Update UI using AsyncApp::update()
            let _ = this.update(cx, |chart, cx| {
                match result {
                    Ok(bars) => {
                        chart.bars = bars;
                        chart.error = None;
                        println!(
                            "✓ Successfully loaded {} bars for {}",
                            chart.bars.len(),
                            chart.symbol
                        );
                    }
                    Err(error) => {
                        chart.error = Some(error.clone());
                        chart.bars = generate_mock_data();
                        eprintln!("✗ Error fetching bars: {}. Using mock data.", error);
                    }
                }
                chart.loading = false;
                cx.notify();
            });
        })
        .detach();
    }

    fn render_candlesticks(&self) -> impl IntoElement {
        if self.bars.is_empty() {
            let message = if self.loading {
                "Loading data from Alpaca Markets...".to_string()
            } else if let Some(ref error) = self.error {
                error.clone()
            } else {
                "No data available.".to_string()
            };

            return div()
                .flex()
                .items_center()
                .justify_center()
                .size_full()
                .child(div().text_color(rgb(0x808080)).child(message));
        }

        let chart_width = 1200.0_f32;
        let chart_height = 600.0_f32;
        let padding = 60.0_f32;

        // Calculate price range
        let max_price = self
            .bars
            .iter()
            .map(|b| b.close)
            .fold(f64::NEG_INFINITY, f64::max);
        let min_price = self
            .bars
            .iter()
            .map(|b| b.close)
            .fold(f64::INFINITY, f64::min);

        let price_range = max_price - min_price;
        let price_padding = price_range * 0.1;
        let adjusted_max = max_price + price_padding;
        let adjusted_min = min_price - price_padding;
        let adjusted_range = adjusted_max - adjusted_min;

        let candle_width = (chart_width - 2.0 * padding) / self.bars.len() as f32;
        let candle_spacing = candle_width * 0.3;
        let actual_candle_width = (candle_width - candle_spacing).max(2.0);

        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                // Chart container
                div()
                    .w(px(chart_width))
                    .h(px(chart_height))
                    .bg(rgb(0x1a1a1a))
                    .border_2()
                    .border_color(rgb(0x404040))
                    .relative()
                    .overflow_hidden()
                    // Price grid lines
                    .children((0..6).map(|i| {
                        let y = padding + (i as f32 / 5.0) * (chart_height - 2.0 * padding);
                        let price = adjusted_max - (i as f64 / 5.0) * adjusted_range;

                        div()
                            .absolute()
                            .left(px(0.0))
                            .top(px(y))
                            .w_full()
                            .h(px(1.0))
                            .bg(rgb(0x2a2a2a))
                            .child(
                                div()
                                    .absolute()
                                    .left(px(5.0))
                                    .top(px(-8.0))
                                    .text_xs()
                                    .text_color(rgb(0x808080))
                                    .child(format!("${:.2}", price)),
                            )
                    }))
                    // Candlesticks
                    .children(self.bars.iter().enumerate().map(|(i, bar)| {
                        let x = padding + i as f32 * candle_width;

                        // Calculate Y positions (inverted because canvas origin is top-left)
                        let high_y = padding
                            + ((adjusted_max - bar.high) / adjusted_range) as f32
                                * (chart_height - 2.0 * padding);
                        let low_y = padding
                            + ((adjusted_max - bar.low) / adjusted_range) as f32
                                * (chart_height - 2.0 * padding);
                        let open_y = padding
                            + ((adjusted_max - bar.open) / adjusted_range) as f32
                                * (chart_height - 2.0 * padding);
                        let close_y = padding
                            + ((adjusted_max - bar.close) / adjusted_range) as f32
                                * (chart_height - 2.0 * padding);

                        let body_top = open_y.min(close_y);
                        let body_height = (open_y - close_y).abs().max(1.0);

                        // Determine if bullish or bearish
                        let is_bullish = bar.close >= bar.open;
                        let (color, fill_color) = if is_bullish {
                            (rgb(0x00cc66), rgb(0x00cc66))
                        } else {
                            (rgb(0xff4444), rgb(0xff4444))
                        };

                        div()
                            .absolute()
                            // High-Low wick (thin line)
                            .child(
                                div()
                                    .absolute()
                                    .left(px(
                                        x + candle_spacing / 2.0 + actual_candle_width / 2.0 - 0.5
                                    ))
                                    .top(px(high_y))
                                    .w(px(1.0))
                                    .h(px(low_y - high_y))
                                    .bg(color),
                            )
                            // Open-Close body (thicker rectangle)
                            .child(
                                div()
                                    .absolute()
                                    .left(px(x + candle_spacing / 2.0))
                                    .top(px(body_top))
                                    .w(px(actual_candle_width))
                                    .h(px(body_height))
                                    .bg(fill_color)
                                    .border_1()
                                    .border_color(color),
                            )
                    })),
            )
            .child(
                // Price statistics
                div()
                    .flex()
                    .gap_6()
                    .text_sm()
                    .text_color(rgb(0xcccccc))
                    .child(div().child(format!("High: ${:.2}", max_price)))
                    .child(div().child(format!("Low: ${:.2}", min_price)))
                    .child(div().child(format!("Range: ${:.2}", price_range)))
                    .child(div().child(format!("Bars: {}", self.bars.len())))
                    .when_some(self.bars.last(), |this, last_bar| {
                        let is_bullish = last_bar.close >= last_bar.open;
                        let color = if is_bullish {
                            rgb(0x00cc66)
                        } else {
                            rgb(0xff4444)
                        };
                        this.child(
                            div()
                                .text_color(color)
                                .child(format!("Last Close: ${:.2}", last_bar.close)),
                        )
                    }),
            )
    }
}

impl Render for BarChart {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let timeframe_display = match self.timeframe.as_str() {
            "1Min" => "1 Minute",
            "5Min" => "5 Minutes",
            "15Min" => "15 Minutes",
            "1Hour" => "1 Hour",
            "1Day" => "Daily",
            "1Week" => "Weekly",
            "1Month" => "Monthly",
            _ => &self.timeframe,
        };

        div()
            .flex()
            .bg(rgb(0x0d1117))
            .size_full()
            .child(
                // Main content area
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .p_8()
                    .gap_6()
                    .track_focus(&self.focus_handle)
                    .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, _, cx| {
                        // Handle symbol input
                        if this.input_focused {
                            let key = event.keystroke.key.as_str();

                            if key == "enter" {
                                this.submit_symbol(cx);
                            } else if key == "backspace" {
                                this.handle_backspace(cx);
                            } else if key == "escape" {
                                this.input_focused = false;
                                cx.notify();
                            } else if let Some(key_char) = &event.keystroke.key_char {
                                if key_char.len() == 1
                                    && key_char.chars().all(|c| c.is_alphanumeric())
                                {
                                    this.handle_input(key_char, cx);
                                }
                            }
                            return;
                        }

                        // Handle quantity input
                        if this.quantity_focused {
                            let key = event.keystroke.key.as_str();

                            if key == "enter" {
                                this.quantity_focused = false;
                                cx.notify();
                            } else if key == "backspace" {
                                this.order_quantity.pop();
                                cx.notify();
                            } else if key == "escape" {
                                this.quantity_focused = false;
                                cx.notify();
                            } else if let Some(key_char) = &event.keystroke.key_char {
                                if key_char.len() == 1
                                    && (key_char.chars().all(|c| c.is_numeric()) || key_char == ".")
                                {
                                    this.order_quantity.push_str(key_char);
                                    cx.notify();
                                }
                            }
                            return;
                        }

                        // Handle price input
                        if this.price_focused {
                            let key = event.keystroke.key.as_str();

                            if key == "enter" {
                                this.price_focused = false;
                                cx.notify();
                            } else if key == "backspace" {
                                this.order_limit_price.pop();
                                cx.notify();
                            } else if key == "escape" {
                                this.price_focused = false;
                                cx.notify();
                            } else if let Some(key_char) = &event.keystroke.key_char {
                                if key_char.len() == 1
                                    && (key_char.chars().all(|c| c.is_numeric()) || key_char == ".")
                                {
                                    this.order_limit_price.push_str(key_char);
                                    cx.notify();
                                }
                            }
                            return;
                        }
                    }))
                    .child(
                        // Header
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_2xl()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(rgb(0xffffff))
                                            .child(format!("{} Stock Chart", self.symbol)),
                                    )
                                    .child(div().text_sm().text_color(rgb(0x808080)).child(
                                        format!(
                                            "{} candlestick chart powered by Alpaca Markets",
                                            timeframe_display
                                        ),
                                    )),
                            )
                            .child(
                                div()
                                    .id("refresh-button")
                                    .px_6()
                                    .py_3()
                                    .bg(rgb(0x238636))
                                    .rounded_lg()
                                    .text_color(rgb(0xffffff))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .cursor_pointer()
                                    .hover(|style| style.bg(rgb(0x2ea043)))
                                    .child(if self.loading {
                                        "⟳ Loading..."
                                    } else {
                                        "↻ Refresh Data"
                                    })
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.fetch_bars(cx);
                                    })),
                            )
                            .child(
                                // WebSocket Status Indicator
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .px_4()
                                    .py_3()
                                    .rounded_lg()
                                    .bg(if self.stream_connected {
                                        rgb(0x238636)
                                    } else {
                                        rgb(0x6e7681)
                                    })
                                    .child(div().text_sm().text_color(rgb(0xffffff)).child(
                                        if self.stream_connected {
                                            "🟢 Live Updates"
                                        } else {
                                            "⭕ Disconnected"
                                        },
                                    )),
                            )
                            .child(
                                // Market Data WebSocket Status Indicator
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .px_4()
                                    .py_3()
                                    .rounded_lg()
                                    .bg(if self.market_data_connected {
                                        rgb(0x1f6feb)
                                    } else {
                                        rgb(0x6e7681)
                                    })
                                    .child(div().text_sm().text_color(rgb(0xffffff)).child(
                                        if self.market_data_connected {
                                            if let Some(ref last_time) = self.last_bar_time {
                                                format!(
                                                    "📊 Market Data (Last: {})",
                                                    &last_time[11..19]
                                                ) // Show just HH:MM:SS
                                            } else {
                                                "📊 Market Data".to_string()
                                            }
                                        } else {
                                            "📊 No Market Data".to_string()
                                        },
                                    )),
                            ),
                    )
                    .child(
                        // Controls: Symbol input and Timeframe selector
                        div()
                            .flex()
                            .gap_4()
                            .items_end()
                            .child(
                                // Symbol input
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(rgb(0xffffff))
                                            .child("Symbol:"),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .id("symbol-input")
                                                    .px_4()
                                                    .py_2()
                                                    .bg(if self.input_focused {
                                                        rgb(0x1f2937)
                                                    } else {
                                                        rgb(0x161b22)
                                                    })
                                                    .border_1()
                                                    .border_color(if self.input_focused {
                                                        rgb(0x1f6feb)
                                                    } else {
                                                        rgb(0x30363d)
                                                    })
                                                    .rounded_lg()
                                                    .text_color(rgb(0xffffff))
                                                    .min_w(px(120.0))
                                                    .cursor_text()
                                                    .child(if self.input_focused {
                                                        format!("{}|", self.symbol_input)
                                                    } else if self.symbol_input.is_empty() {
                                                        "Enter symbol...".to_string()
                                                    } else {
                                                        self.symbol_input.clone()
                                                    })
                                                    .on_click(cx.listener(
                                                        |this, _, _window, cx| {
                                                            this.input_focused = true;
                                                            _window.focus(&this.focus_handle);
                                                            cx.notify();
                                                        },
                                                    )),
                                            )
                                            .child(
                                                div()
                                                    .id("update-symbol-button")
                                                    .px_4()
                                                    .py_2()
                                                    .bg(rgb(0x1f6feb))
                                                    .rounded_lg()
                                                    .text_color(rgb(0xffffff))
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .cursor_pointer()
                                                    .hover(|style| style.bg(rgb(0x388bfd)))
                                                    .child("Update")
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.submit_symbol(cx);
                                                    })),
                                            ),
                                    ),
                            )
                            .child(
                                // Timeframe selector
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(rgb(0xffffff))
                                            .child("Timeframe:"),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .gap_2()
                                            .child(self.render_timeframe_button("1Min", "1m", cx))
                                            .child(self.render_timeframe_button("5Min", "5m", cx))
                                            .child(self.render_timeframe_button("15Min", "15m", cx))
                                            .child(self.render_timeframe_button("1Hour", "1h", cx))
                                            .child(self.render_timeframe_button("1Day", "1D", cx))
                                            .child(self.render_timeframe_button("1Week", "1W", cx))
                                            .child(
                                                self.render_timeframe_button("1Month", "1M", cx),
                                            ),
                                    ),
                            ),
                    )
                    .child(
                        // Chart area
                        div()
                            .flex()
                            .flex_1()
                            .items_center()
                            .justify_center()
                            .child(self.render_candlesticks()),
                    )
                    .child(
                        // Tabbed Footer
                        div()
                            .flex()
                            .flex_col()
                            .gap_3()
                            .p_4()
                            .bg(rgb(0x161b22))
                            .rounded_lg()
                            .border_1()
                            .border_color(rgb(0x30363d))
                            .child(
                                // Tab buttons and refresh button
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .child(
                                        div()
                                            .flex()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .id("tab-account")
                                                    .px_4()
                                                    .py_2()
                                                    .rounded_md()
                                                    .text_sm()
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .cursor_pointer()
                                                    .bg(
                                                        if self.active_footer_tab
                                                            == FooterTab::Account
                                                        {
                                                            rgb(0x238636)
                                                        } else {
                                                            rgb(0x21262d)
                                                        },
                                                    )
                                                    .text_color(rgb(0xffffff))
                                                    .hover(|style| {
                                                        if self.active_footer_tab
                                                            == FooterTab::Account
                                                        {
                                                            style.bg(rgb(0x2ea043))
                                                        } else {
                                                            style.bg(rgb(0x30363d))
                                                        }
                                                    })
                                                    .child("Account Information")
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.active_footer_tab = FooterTab::Account;
                                                        cx.notify();
                                                    })),
                                            )
                                            .child(
                                                div()
                                                    .id("tab-positions")
                                                    .px_4()
                                                    .py_2()
                                                    .rounded_md()
                                                    .text_sm()
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .cursor_pointer()
                                                    .bg(
                                                        if self.active_footer_tab
                                                            == FooterTab::Positions
                                                        {
                                                            rgb(0x238636)
                                                        } else {
                                                            rgb(0x21262d)
                                                        },
                                                    )
                                                    .text_color(rgb(0xffffff))
                                                    .hover(|style| {
                                                        if self.active_footer_tab
                                                            == FooterTab::Positions
                                                        {
                                                            style.bg(rgb(0x2ea043))
                                                        } else {
                                                            style.bg(rgb(0x30363d))
                                                        }
                                                    })
                                                    .child("Active Positions")
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.active_footer_tab =
                                                            FooterTab::Positions;
                                                        cx.notify();
                                                    })),
                                            )
                                            .child(
                                                div()
                                                    .id("tab-orders")
                                                    .px_4()
                                                    .py_2()
                                                    .rounded_md()
                                                    .text_sm()
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .cursor_pointer()
                                                    .bg(
                                                        if self.active_footer_tab
                                                            == FooterTab::Orders
                                                        {
                                                            rgb(0x238636)
                                                        } else {
                                                            rgb(0x21262d)
                                                        },
                                                    )
                                                    .text_color(rgb(0xffffff))
                                                    .hover(|style| {
                                                        if self.active_footer_tab
                                                            == FooterTab::Orders
                                                        {
                                                            style.bg(rgb(0x2ea043))
                                                        } else {
                                                            style.bg(rgb(0x30363d))
                                                        }
                                                    })
                                                    .child("Active Orders")
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.active_footer_tab = FooterTab::Orders;
                                                        cx.notify();
                                                    })),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .id("refresh-footer-button")
                                            .px_3()
                                            .py_1()
                                            .bg(rgb(0x238636))
                                            .rounded_md()
                                            .text_xs()
                                            .text_color(rgb(0xffffff))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .cursor_pointer()
                                            .hover(|style| style.bg(rgb(0x2ea043)))
                                            .child(
                                                if (self.active_footer_tab == FooterTab::Account
                                                    && self.account_loading)
                                                    || (self.active_footer_tab
                                                        == FooterTab::Positions
                                                        && self.positions_loading)
                                                    || (self.active_footer_tab == FooterTab::Orders
                                                        && self.orders_loading)
                                                {
                                                    "⟳ Loading..."
                                                } else {
                                                    "↻ Refresh"
                                                },
                                            )
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                match this.active_footer_tab {
                                                    FooterTab::Account => this.fetch_account(cx),
                                                    FooterTab::Positions => {
                                                        this.fetch_positions(cx)
                                                    }
                                                    FooterTab::Orders => this.fetch_orders(cx),
                                                }
                                            })),
                                    ),
                            )
                            .when(self.active_footer_tab == FooterTab::Account, |div| {
                                div.child(self.render_account_tab())
                            })
                            .when(self.active_footer_tab == FooterTab::Positions, |div| {
                                div.child(self.render_positions_tab(cx))
                            })
                            .when(self.active_footer_tab == FooterTab::Orders, |div| {
                                div.child(self.render_orders_tab(cx))
                            }),
                    ),
            )
            .child(
                // Right sidebar - Order form
                div()
                    .w(px(320.0))
                    .h_full()
                    .bg(rgb(0x161b22))
                    .border_l_1()
                    .border_color(rgb(0x30363d))
                    .p_6()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(0xffffff))
                            .child("Place Order"),
                    )
                    .child(
                        // Current symbol display
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(0xffffff))
                                    .child("Trading Symbol"),
                            )
                            .child(
                                div()
                                    .px_3()
                                    .py_2()
                                    .bg(rgb(0x0d1117))
                                    .border_1()
                                    .border_color(rgb(0x1f6feb))
                                    .rounded_md()
                                    .text_color(rgb(0x58a6ff))
                                    .font_weight(FontWeight::BOLD)
                                    .child(self.symbol.clone()),
                            ),
                    )
                    .child(
                        // Order side (Buy/Sell)
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(0xffffff))
                                    .child("Side"),
                            )
                            .child(
                                div()
                                    .flex()
                                    .gap_2()
                                    .child(
                                        div()
                                            .id("order-side-buy")
                                            .flex_1()
                                            .px_3()
                                            .py_2()
                                            .rounded_md()
                                            .text_center()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .cursor_pointer()
                                            .bg(if matches!(self.order_side, OrderSide::Buy) {
                                                rgb(0x238636)
                                            } else {
                                                rgb(0x21262d)
                                            })
                                            .text_color(rgb(0xffffff))
                                            .hover(|style| {
                                                if matches!(self.order_side, OrderSide::Buy) {
                                                    style.bg(rgb(0x2ea043))
                                                } else {
                                                    style.bg(rgb(0x30363d))
                                                }
                                            })
                                            .child("Buy")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.order_side = OrderSide::Buy;
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        div()
                                            .id("order-side-sell")
                                            .flex_1()
                                            .px_3()
                                            .py_2()
                                            .rounded_md()
                                            .text_center()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .cursor_pointer()
                                            .bg(if matches!(self.order_side, OrderSide::Sell) {
                                                rgb(0xda3633)
                                            } else {
                                                rgb(0x21262d)
                                            })
                                            .text_color(rgb(0xffffff))
                                            .hover(|style| {
                                                if matches!(self.order_side, OrderSide::Sell) {
                                                    style.bg(rgb(0xff4444))
                                                } else {
                                                    style.bg(rgb(0x30363d))
                                                }
                                            })
                                            .child("Sell")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.order_side = OrderSide::Sell;
                                                cx.notify();
                                            })),
                                    ),
                            ),
                    )
                    .child(
                        // Order type (Market/Limit)
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(0xffffff))
                                    .child("Order Type"),
                            )
                            .child(
                                div()
                                    .flex()
                                    .gap_2()
                                    .child(
                                        div()
                                            .id("order-type-market")
                                            .flex_1()
                                            .px_3()
                                            .py_2()
                                            .rounded_md()
                                            .text_center()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .cursor_pointer()
                                            .bg(if matches!(self.order_type, OrderType::Market) {
                                                rgb(0x1f6feb)
                                            } else {
                                                rgb(0x21262d)
                                            })
                                            .text_color(rgb(0xffffff))
                                            .hover(|style| {
                                                if matches!(self.order_type, OrderType::Market) {
                                                    style.bg(rgb(0x388bfd))
                                                } else {
                                                    style.bg(rgb(0x30363d))
                                                }
                                            })
                                            .child("Market")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.order_type = OrderType::Market;
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        div()
                                            .id("order-type-limit")
                                            .flex_1()
                                            .px_3()
                                            .py_2()
                                            .rounded_md()
                                            .text_center()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .cursor_pointer()
                                            .bg(if matches!(self.order_type, OrderType::Limit) {
                                                rgb(0x1f6feb)
                                            } else {
                                                rgb(0x21262d)
                                            })
                                            .text_color(rgb(0xffffff))
                                            .hover(|style| {
                                                if matches!(self.order_type, OrderType::Limit) {
                                                    style.bg(rgb(0x388bfd))
                                                } else {
                                                    style.bg(rgb(0x30363d))
                                                }
                                            })
                                            .child("Limit")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.order_type = OrderType::Limit;
                                                cx.notify();
                                            })),
                                    ),
                            ),
                    )
                    .child(
                        // Quantity input
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(0xffffff))
                                    .child("Quantity"),
                            )
                            .child(
                                div()
                                    .id("order-quantity-input")
                                    .px_3()
                                    .py_2()
                                    .bg(if self.quantity_focused {
                                        rgb(0x1f2937)
                                    } else {
                                        rgb(0x0d1117)
                                    })
                                    .border_1()
                                    .border_color(if self.quantity_focused {
                                        rgb(0x1f6feb)
                                    } else {
                                        rgb(0x30363d)
                                    })
                                    .rounded_md()
                                    .text_color(rgb(0xffffff))
                                    .cursor_text()
                                    .child(if self.quantity_focused {
                                        format!("{}|", self.order_quantity)
                                    } else if self.order_quantity.is_empty() {
                                        "Enter quantity...".to_string()
                                    } else {
                                        self.order_quantity.clone()
                                    })
                                    .on_click(cx.listener(|this, _, _window, cx| {
                                        this.quantity_focused = true;
                                        this.input_focused = false;
                                        this.price_focused = false;
                                        _window.focus(&this.focus_handle);
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(
                        // Limit price input (shown only for limit orders)
                        self.render_limit_price_input(cx),
                    )
                    .child(
                        // Time in Force (shown only for limit orders)
                        self.render_time_in_force(cx),
                    )
                    .child(
                        // Submit button
                        div()
                            .id("submit-order-button")
                            .px_4()
                            .py_3()
                            .mt_4()
                            .bg(if matches!(self.order_side, OrderSide::Buy) {
                                rgb(0x238636)
                            } else {
                                rgb(0xda3633)
                            })
                            .rounded_md()
                            .text_center()
                            .text_color(rgb(0xffffff))
                            .font_weight(FontWeight::BOLD)
                            .cursor_pointer()
                            .hover(|style| {
                                if matches!(self.order_side, OrderSide::Buy) {
                                    style.bg(rgb(0x2ea043))
                                } else {
                                    style.bg(rgb(0xff4444))
                                }
                            })
                            .child(if self.order_submitting {
                                "Submitting...".to_string()
                            } else {
                                format!(
                                    "{} {}",
                                    if matches!(self.order_side, OrderSide::Buy) {
                                        "Buy"
                                    } else {
                                        "Sell"
                                    },
                                    self.symbol
                                )
                            })
                            .on_click(cx.listener(|this, _, _, cx| {
                                if !this.order_submitting {
                                    this.submit_order(cx);
                                }
                            })),
                    )
                    .child(self.render_order_message(cx)),
            )
    }
}

impl BarChart {
    fn render_account_tab(&self) -> impl IntoElement {
        div()
            .flex()
            .gap_6()
            .text_sm()
            .child(
                self.render_account_stat(
                    "Account Number".to_string(),
                    self.account_number
                        .clone()
                        .unwrap_or("Loading...".to_string()),
                    rgb(0xa371f7),
                ),
            )
            .child(
                self.render_account_stat(
                    "Account Status".to_string(),
                    self.account_status
                        .clone()
                        .unwrap_or("Loading...".to_string()),
                    rgb(0x58a6ff),
                ),
            )
            .child(self.render_account_stat(
                "Portfolio Value".to_string(),
                format!("${:.2}", self.portfolio_value.unwrap_or(0.0)),
                rgb(0x3fb950),
            ))
            .child(self.render_account_stat(
                "Equity".to_string(),
                format!("${:.2}", self.equity.unwrap_or(0.0)),
                rgb(0x3fb950),
            ))
            .child(self.render_account_stat(
                "Cash".to_string(),
                format!("${:.2}", self.cash.unwrap_or(0.0)),
                rgb(0xf2cc60),
            ))
            .child(self.render_account_stat(
                "Buying Power".to_string(),
                format!("${:.2}", self.buying_power.unwrap_or(0.0)),
                rgb(0xf2cc60),
            ))
    }

    fn render_positions_tab(&self, cx: &mut Context<Self>) -> impl IntoElement {
        if self.positions_loading {
            return div()
                .flex()
                .items_center()
                .justify_center()
                .p_6()
                .text_color(rgb(0x8b949e))
                .child("Loading positions...");
        }

        if self.positions.is_empty() {
            return div()
                .flex()
                .items_center()
                .justify_center()
                .p_6()
                .text_color(rgb(0x8b949e))
                .child("No active positions");
        }

        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                // Table header
                div()
                    .flex()
                    .gap_4()
                    .pb_2()
                    .border_b_1()
                    .border_color(rgb(0x30363d))
                    .child(
                        div()
                            .w(px(80.0))
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(0x8b949e))
                            .child("Symbol"),
                    )
                    .child(
                        div()
                            .w(px(80.0))
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(0x8b949e))
                            .child("Qty"),
                    )
                    .child(
                        div()
                            .w(px(100.0))
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(0x8b949e))
                            .child("Avg Entry"),
                    )
                    .child(
                        div()
                            .w(px(100.0))
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(0x8b949e))
                            .child("Current"),
                    )
                    .child(
                        div()
                            .w(px(120.0))
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(0x8b949e))
                            .child("Market Value"),
                    )
                    .child(
                        div()
                            .w(px(100.0))
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(0x8b949e))
                            .child("P&L"),
                    )
                    .child(
                        div()
                            .w(px(80.0))
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(0x8b949e))
                            .child("P&L %"),
                    )
                    .child(
                        div()
                            .w(px(80.0))
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(0x8b949e))
                            .child("Action"),
                    ),
            )
            .children(self.positions.iter().enumerate().map(|(idx, pos)| {
                let pl_value = pos.unrealized_pl.parse::<f64>().unwrap_or(0.0);
                let pl_color = if pl_value > 0.0 {
                    rgb(0x3fb950)
                } else if pl_value < 0.0 {
                    rgb(0xff4444)
                } else {
                    rgb(0x8b949e)
                };

                div()
                    .flex()
                    .gap_4()
                    .py_2()
                    .child(
                        div()
                            .w(px(80.0))
                            .text_sm()
                            .text_color(rgb(0xffffff))
                            .child(pos.symbol.clone()),
                    )
                    .child(
                        div()
                            .w(px(80.0))
                            .text_sm()
                            .text_color(rgb(0x8b949e))
                            .child(pos.qty.clone()),
                    )
                    .child(
                        div()
                            .w(px(100.0))
                            .text_sm()
                            .text_color(rgb(0x8b949e))
                            .child(format!("${}", pos.avg_entry_price)),
                    )
                    .child(
                        div()
                            .w(px(100.0))
                            .text_sm()
                            .text_color(rgb(0x8b949e))
                            .child(format!("${}", pos.current_price)),
                    )
                    .child(
                        div()
                            .w(px(120.0))
                            .text_sm()
                            .text_color(rgb(0xffffff))
                            .child(format!("${}", pos.market_value)),
                    )
                    .child(
                        div()
                            .w(px(100.0))
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(pl_color)
                            .child(format!("${}", pos.unrealized_pl)),
                    )
                    .child(
                        div()
                            .w(px(80.0))
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(pl_color)
                            .child(format!("{}%", pos.unrealized_plpc)),
                    )
                    .child(
                        div().w(px(80.0)).child(
                            div()
                                .id(ElementId::Name(format!("close-position-{}", idx).into()))
                                .px_3()
                                .py_1()
                                .bg(rgb(0xf2cc60))
                                .rounded_md()
                                .text_xs()
                                .text_color(rgb(0x000000))
                                .font_weight(FontWeight::SEMIBOLD)
                                .cursor_pointer()
                                .hover(|style| style.bg(rgb(0xffd700)))
                                .child("Close")
                                .on_click({
                                    let symbol = pos.symbol.clone();
                                    cx.listener(move |this, _, _, cx| {
                                        this.close_position(symbol.clone(), cx);
                                    })
                                }),
                        ),
                    )
            }))
    }

    fn render_orders_tab(&self, cx: &mut Context<Self>) -> impl IntoElement {
        if self.orders_loading {
            return div()
                .flex()
                .items_center()
                .justify_center()
                .p_6()
                .text_color(rgb(0x8b949e))
                .child("Loading orders...");
        }

        if self.orders.is_empty() {
            return div()
                .flex()
                .items_center()
                .justify_center()
                .p_6()
                .text_color(rgb(0x8b949e))
                .child("No active orders");
        }

        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                // Table header
                div()
                    .flex()
                    .gap_4()
                    .pb_2()
                    .border_b_1()
                    .border_color(rgb(0x30363d))
                    .child(
                        div()
                            .w(px(80.0))
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(0x8b949e))
                            .child("Symbol"),
                    )
                    .child(
                        div()
                            .w(px(60.0))
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(0x8b949e))
                            .child("Side"),
                    )
                    .child(
                        div()
                            .w(px(80.0))
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(0x8b949e))
                            .child("Qty"),
                    )
                    .child(
                        div()
                            .w(px(80.0))
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(0x8b949e))
                            .child("Type"),
                    )
                    .child(
                        div()
                            .w(px(100.0))
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(0x8b949e))
                            .child("Limit Price"),
                    )
                    .child(
                        div()
                            .w(px(100.0))
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(0x8b949e))
                            .child("Status"),
                    )
                    .child(
                        div()
                            .w(px(150.0))
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(0x8b949e))
                            .child("Created At"),
                    )
                    .child(
                        div()
                            .w(px(80.0))
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(0x8b949e))
                            .child("Action"),
                    ),
            )
            .children(self.orders.iter().enumerate().map(|(idx, order)| {
                let side_color = if order.side.to_lowercase().contains("buy") {
                    rgb(0x3fb950)
                } else {
                    rgb(0xff4444)
                };

                let status_color = match order.status.to_lowercase().as_str() {
                    s if s.contains("filled") => rgb(0x3fb950),
                    s if s.contains("canceled") || s.contains("rejected") => rgb(0xff4444),
                    s if s.contains("pending") => rgb(0xf2cc60),
                    _ => rgb(0x58a6ff),
                };

                div()
                    .flex()
                    .gap_4()
                    .py_2()
                    .child(
                        div()
                            .w(px(80.0))
                            .text_sm()
                            .text_color(rgb(0xffffff))
                            .child(order.symbol.clone()),
                    )
                    .child(
                        div()
                            .w(px(60.0))
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(side_color)
                            .child(order.side.clone()),
                    )
                    .child(
                        div()
                            .w(px(80.0))
                            .text_sm()
                            .text_color(rgb(0x8b949e))
                            .child(order.qty.clone()),
                    )
                    .child(
                        div()
                            .w(px(80.0))
                            .text_sm()
                            .text_color(rgb(0x8b949e))
                            .child(order.order_type.clone()),
                    )
                    .child(
                        div()
                            .w(px(100.0))
                            .text_sm()
                            .text_color(rgb(0x8b949e))
                            .child(order.limit_price.clone().unwrap_or("-".to_string())),
                    )
                    .child(
                        div()
                            .w(px(100.0))
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(status_color)
                            .child(order.status.clone()),
                    )
                    .child(
                        div()
                            .w(px(150.0))
                            .text_sm()
                            .text_color(rgb(0x8b949e))
                            .child(order.created_at.clone()),
                    )
                    .child(
                        div().w(px(80.0)).child(
                            div()
                                .id(ElementId::Name(format!("cancel-order-{}", idx).into()))
                                .px_3()
                                .py_1()
                                .bg(rgb(0xda3633))
                                .rounded_md()
                                .text_xs()
                                .text_color(rgb(0xffffff))
                                .font_weight(FontWeight::SEMIBOLD)
                                .cursor_pointer()
                                .hover(|style| style.bg(rgb(0xff4444)))
                                .child("Cancel")
                                .on_click({
                                    let order_id = order.id.clone();
                                    cx.listener(move |this, _, _, cx| {
                                        this.cancel_order(order_id.clone(), cx);
                                    })
                                }),
                        ),
                    )
            }))
    }

    fn render_limit_price_input(&self, cx: &mut Context<Self>) -> impl IntoElement {
        if !matches!(self.order_type, OrderType::Limit) {
            return div();
        }

        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(0xffffff))
                    .child("Limit Price"),
            )
            .child(
                div()
                    .id("order-limit-price-input")
                    .px_3()
                    .py_2()
                    .bg(if self.price_focused {
                        rgb(0x1f2937)
                    } else {
                        rgb(0x0d1117)
                    })
                    .border_1()
                    .border_color(if self.price_focused {
                        rgb(0x1f6feb)
                    } else {
                        rgb(0x30363d)
                    })
                    .rounded_md()
                    .text_color(rgb(0xffffff))
                    .cursor_text()
                    .child(if self.price_focused {
                        format!("{}|", self.order_limit_price)
                    } else if self.order_limit_price.is_empty() {
                        "Enter price...".to_string()
                    } else {
                        format!("${}", self.order_limit_price)
                    })
                    .on_click(cx.listener(|this, _, _window, cx| {
                        this.price_focused = true;
                        this.input_focused = false;
                        this.quantity_focused = false;
                        _window.focus(&this.focus_handle);
                        cx.notify();
                    })),
            )
    }

    fn render_time_in_force(&self, cx: &mut Context<Self>) -> impl IntoElement {
        if !matches!(self.order_type, OrderType::Limit) {
            return div();
        }

        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(0xffffff))
                    .child("Time in Force"),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        div()
                            .id("tif-day-btn")
                            .flex_1()
                            .px_3()
                            .py_2()
                            .rounded_md()
                            .text_center()
                            .font_weight(FontWeight::SEMIBOLD)
                            .cursor_pointer()
                            .bg(
                                if matches!(self.order_time_in_force, OrderTimeInForce::Day) {
                                    rgb(0x1f6feb)
                                } else {
                                    rgb(0x21262d)
                                },
                            )
                            .text_color(rgb(0xffffff))
                            .hover(|style| {
                                if matches!(self.order_time_in_force, OrderTimeInForce::Day) {
                                    style.bg(rgb(0x388bfd))
                                } else {
                                    style.bg(rgb(0x30363d))
                                }
                            })
                            .child("Day")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.order_time_in_force = OrderTimeInForce::Day;
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .id("tif-gtc-btn")
                            .flex_1()
                            .px_3()
                            .py_2()
                            .rounded_md()
                            .text_center()
                            .font_weight(FontWeight::SEMIBOLD)
                            .cursor_pointer()
                            .bg(
                                if matches!(self.order_time_in_force, OrderTimeInForce::Gtc) {
                                    rgb(0x1f6feb)
                                } else {
                                    rgb(0x21262d)
                                },
                            )
                            .text_color(rgb(0xffffff))
                            .hover(|style| {
                                if matches!(self.order_time_in_force, OrderTimeInForce::Gtc) {
                                    style.bg(rgb(0x388bfd))
                                } else {
                                    style.bg(rgb(0x30363d))
                                }
                            })
                            .child("GTC")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.order_time_in_force = OrderTimeInForce::Gtc;
                                cx.notify();
                            })),
                    ),
            )
    }

    fn render_order_message(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        if self.order_message.is_none() {
            return div();
        }

        div()
            .px_3()
            .py_2()
            .bg(rgb(0x21262d))
            .border_1()
            .border_color(rgb(0x30363d))
            .rounded_md()
            .text_xs()
            .text_color(if self.order_message.as_ref().unwrap().starts_with("✓") {
                rgb(0x3fb950)
            } else {
                rgb(0xff4444)
            })
            .child(self.order_message.clone().unwrap())
    }

    fn render_account_stat(
        &self,
        label: String,
        value: String,
        color: gpui::Rgba,
    ) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(0x8b949e))
                    .child(label.clone()),
            )
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(color)
                    .child(value.clone()),
            )
    }

    fn render_timeframe_button(
        &self,
        timeframe: &str,
        label: &str,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let is_selected = self.timeframe == timeframe;
        let timeframe_owned = timeframe.to_string();
        let label_owned = label.to_string();
        let element_id = format!("timeframe-{}", timeframe);

        div()
            .id(ElementId::Name(element_id.into()))
            .px_3()
            .py_2()
            .rounded_lg()
            .text_color(if is_selected {
                rgb(0xffffff)
            } else {
                rgb(0x8b949e)
            })
            .bg(if is_selected {
                rgb(0x1f6feb)
            } else {
                rgb(0x161b22)
            })
            .border_1()
            .border_color(if is_selected {
                rgb(0x1f6feb)
            } else {
                rgb(0x30363d)
            })
            .font_weight(if is_selected {
                FontWeight::SEMIBOLD
            } else {
                FontWeight::NORMAL
            })
            .cursor_pointer()
            .hover(|style| {
                if is_selected {
                    style.bg(rgb(0x388bfd))
                } else {
                    style.bg(rgb(0x21262d))
                }
            })
            .child(label_owned)
            .on_click(cx.listener(move |this, _, _, cx| {
                this.timeframe = timeframe_owned.clone();
                this.fetch_bars(cx);
            }))
    }
}

// Synchronous function to fetch account info (runs in background thread)
fn fetch_account_sync() -> Result<(String, String, f64, f64, f64, f64), String> {
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

// Synchronous function to fetch positions (runs in background thread)
fn fetch_positions_sync() -> Result<Vec<Position>, String> {
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

// Synchronous function to fetch orders (runs in background thread)
fn fetch_orders_sync() -> Result<Vec<Order>, String> {
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

// Synchronous function to submit an order (runs in background thread)
fn submit_order_sync(
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

        use alpaca_markets::models::OrderRequest;

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

// Synchronous function to cancel an order (runs in background thread)
fn cancel_order_sync(order_id: String) -> Result<(), String> {
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

// Synchronous function to close a position (runs in background thread)
fn close_position_sync(symbol: String) -> Result<(), String> {
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

// Synchronous function to fetch bars (runs in background thread)
fn fetch_bars_sync(symbol: &str, timeframe: &str) -> Result<Vec<Bar>, String> {
    let rt = tokio::runtime::Runtime::new().map_err(|e| format!("Runtime error: {:?}", e))?;

    rt.block_on(async {
        // Load configuration from environment
        let config = match AlpacaConfig::from_env() {
            Ok(config) => config.with_iex_feed(),
            Err(e) => {
                return Err(format!(
                    "Error loading config: {:?}. Please set APCA_API_KEY_ID and APCA_API_SECRET_KEY environment variables.",
                    e
                ));
            }
        };

        let client = MarketDataClient::new(config);

        // Fetch bars based on timeframe
        let end_time = Utc::now();
        let (start_time, limit) = match timeframe {
            "1Min" => (end_time - Duration::hours(24), Some(100)),
            "5Min" => (end_time - Duration::days(5), Some(100)),
            "15Min" => (end_time - Duration::days(10), Some(100)),
            "1Hour" => (end_time - Duration::days(30), Some(100)),
            "1Day" => (end_time - Duration::days(200), Some(100)),
            "1Week" => (end_time - Duration::days(700), Some(100)),
            "1Month" => (end_time - Duration::days(2500), Some(100)),
            _ => (end_time - Duration::days(200), Some(100)),
        };

        let result = client
            .get_bars(symbol, timeframe, Some(start_time), Some(end_time), limit)
            .await;

        match result {
            Ok(bars_response) => Ok(bars_response.bars),
            Err(e) => Err(format!("Error fetching data: {:?}", e)),
        }
    })
}

// Generate mock data for demonstration
fn generate_mock_data() -> Vec<Bar> {
    let mut bars = Vec::new();
    let base_price = 150.0;
    let start_time = Utc::now() - Duration::days(50);

    for i in 0..50 {
        let variation = ((i as f64 * 0.5).sin() * 10.0) + ((i as f64 * 0.1).cos() * 5.0);
        let base = base_price + variation + (i as f64 * 0.2);

        // Generate OHLC values
        let open = base + ((i as f64 * 0.3).sin() * 2.0);
        let high = base.max(open) + (i as f64 * 0.1).abs() + 1.0;
        let low = base.min(open) - (i as f64 * 0.15).abs() - 1.0;
        let close = base;

        let volume = 50_000_000 + (i * 1_000_000) as u64;
        let timestamp = start_time + Duration::days(i as i64);

        bars.push(Bar {
            timestamp,
            open,
            high,
            low,
            close,
            volume,
            trade_count: Some(10000 + i as u64 * 100),
            vwap: Some((high + low + close) / 3.0),
        });
    }

    bars
}

fn main() {
    Application::new().run(|cx: &mut App| {
        cx.activate(true);
        cx.on_action(|_: &Quit, cx| cx.quit());

        cx.open_window(WindowOptions::default(), |_, cx| cx.new(BarChart::new))
            .unwrap();
    });
}
