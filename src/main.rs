use alpaca_markets::{
    Adjustment, AlpacaConfig, Bar, MarketDataClient, Sort, TradingClient,
    models::{OrderSide, OrderTimeInForce, OrderType},
};
use chrono::{Duration, Utc};
use gpui::{
    App, Application, Context, ElementId, FocusHandle, FontWeight, IntoElement, Render, Window,
    WindowOptions, actions, div, prelude::*, px, rgb,
};

mod chart;
mod stream;

use chart::Chart;
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

struct TradingTerminal {
    // Chart state
    chart: Chart,
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
}

impl TradingTerminal {
    fn new(cx: &mut Context<Self>) -> Self {
        let mut terminal = Self {
            chart: Chart::new("AAPL".to_string(), "1Day".to_string()),
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
        };

        // Fetch data on startup
        terminal.fetch_bars(cx);
        terminal.fetch_account(cx);
        terminal.fetch_positions(cx);
        terminal.start_websocket_stream(cx);
        terminal.start_market_data_stream(cx);
        terminal.fetch_orders(cx);
        terminal
    }

    fn handle_input(&mut self, text: &str, cx: &mut Context<Self>) {
        if !self.chart.input_focused {
            return;
        }

        self.chart.symbol_input.push_str(text);
        cx.notify();
    }

    fn handle_backspace(&mut self, cx: &mut Context<Self>) {
        if !self.chart.input_focused {
            return;
        }

        self.chart.symbol_input.pop();
        cx.notify();
    }

    fn submit_symbol(&mut self, cx: &mut Context<Self>) {
        if !self.chart.symbol_input.is_empty() {
            self.chart.symbol = self.chart.symbol_input.clone().to_uppercase();
            self.chart.input_focused = false;
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

        let symbol = self.chart.symbol.clone();
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
                self.chart.market_data_connected = true;
                cx.notify();
            }
            StreamUpdate::MarketDataDisconnected => {
                println!("❌ Market Data WebSocket disconnected");
                self.chart.market_data_connected = false;
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
        let symbol = self.chart.symbol.clone();

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
        // Store the bar update information for display
        self.chart.last_bar_time = Some(bar_update.timestamp.clone());
        self.chart.last_bar_symbol = Some(bar_update.symbol.clone());
        self.chart.last_bar_open = Some(bar_update.open.clone());
        self.chart.last_bar_high = Some(bar_update.high.clone());
        self.chart.last_bar_low = Some(bar_update.low.clone());
        self.chart.last_bar_close = Some(bar_update.close.clone());
        self.chart.last_bar_volume = Some(bar_update.volume.clone());

        println!(
            "📊 Bar Update: {} @ {} - O:{} H:{} L:{} C:{} V:{}",
            bar_update.symbol,
            bar_update.timestamp,
            bar_update.open,
            bar_update.high,
            bar_update.low,
            bar_update.close,
            bar_update.volume,
        );

        // Only update chart if the bar is for the current symbol
        if bar_update.symbol == self.chart.symbol {
            // Convert BarUpdate to Bar struct
            match chart::convert_bar_update_to_bar(&bar_update) {
                Ok(new_bar) => {
                    if self.chart.bars.is_empty() {
                        // No existing bars, just add the new one
                        self.chart.bars.push(new_bar);
                        println!("✅ Added first bar to chart");
                    } else {
                        // Align the incoming bar timestamp to the chart's timeframe
                        let aligned_timestamp = chart::align_timestamp_to_timeframe(
                            new_bar.timestamp,
                            &self.chart.timeframe,
                        );

                        // Get the last bar's timestamp before taking mutable reference
                        let last_bar_timestamp = self.chart.bars.last().unwrap().timestamp;
                        let last_bar_aligned = chart::align_timestamp_to_timeframe(
                            last_bar_timestamp,
                            &self.chart.timeframe,
                        );

                        if aligned_timestamp == last_bar_aligned {
                            // Get mutable reference after calculating timestamps
                            let last_bar = self.chart.bars.last_mut().unwrap();
                            // This bar update belongs to the same timeframe candle as the last bar
                            // Update the last bar by aggregating the data
                            println!(
                                "🔄 Updating existing {} candle (period: {})",
                                self.chart.timeframe,
                                aligned_timestamp.format("%Y-%m-%d %H:%M:%S")
                            );

                            // Keep the open from the existing bar (first price of the period)
                            // Update high to be the maximum
                            last_bar.high = last_bar.high.max(new_bar.high);
                            // Update low to be the minimum
                            last_bar.low = last_bar.low.min(new_bar.low);
                            // Update close to the latest close
                            last_bar.close = new_bar.close;
                            // Add the volume
                            last_bar.volume += new_bar.volume;
                            // Update timestamp to the latest
                            last_bar.timestamp = new_bar.timestamp;
                            // Update optional fields
                            if let (Some(existing_tc), Some(new_tc)) =
                                (last_bar.trade_count, new_bar.trade_count)
                            {
                                last_bar.trade_count = Some(existing_tc + new_tc);
                            }

                            println!(
                                "✅ Updated current {} bar: O:{:.2} H:{:.2} L:{:.2} C:{:.2} V:{}",
                                self.chart.timeframe,
                                last_bar.open,
                                last_bar.high,
                                last_bar.low,
                                last_bar.close,
                                last_bar.volume
                            );
                        } else if aligned_timestamp > last_bar_aligned {
                            // Get mutable reference is not needed here, just push
                            // This is a new timeframe period - append a new bar
                            println!(
                                "➕ New {} candle period started: {}",
                                self.chart.timeframe,
                                aligned_timestamp.format("%Y-%m-%d %H:%M:%S")
                            );
                            self.chart.bars.push(new_bar);
                            println!(
                                "✅ Added new {} bar to chart (total: {})",
                                self.chart.timeframe,
                                self.chart.bars.len()
                            );

                            // Auto-scroll to show the latest bar
                            if self.chart.bars.len() > self.chart.bars_per_screen {
                                self.chart.chart_scroll_offset =
                                    (self.chart.bars.len() - self.chart.bars_per_screen) as f32;
                            }
                        } else {
                            println!("⚠️ Received bar with older timeframe period, ignoring");
                        }
                    }
                }
                Err(e) => {
                    eprintln!("❌ Failed to convert bar update: {}", e);
                }
            }
        }

        // Notify to update the UI
        cx.notify();
    }

    fn fetch_bars(&mut self, cx: &mut Context<Self>) {
        self.chart.loading = true;
        self.chart.error = None;
        cx.notify();

        let symbol = self.chart.symbol.clone();
        let timeframe = self.chart.timeframe.clone();
        let limit = self.chart.bar_limit.parse::<u32>().unwrap_or(100);

        // Modern GPUI async pattern with AsyncApp::update()
        cx.spawn(async move |this, cx| {
            // Run the blocking API call in a background thread
            let result = cx
                .background_executor()
                .spawn(async move { fetch_bars_sync(&symbol, &timeframe, limit) })
                .await;

            // Update UI using AsyncApp::update()
            let _ = this.update(cx, |terminal, cx| {
                match result {
                    Ok(bars) => {
                        terminal.chart.bars = bars;
                        terminal.chart.error = None;
                        // Set scroll offset to show most recent bars by default
                        terminal.chart.chart_scroll_offset = terminal
                            .chart
                            .bars
                            .len()
                            .saturating_sub(terminal.chart.bars_per_screen)
                            as f32;
                        println!(
                            "✓ Successfully loaded {} bars for {} ({})",
                            terminal.chart.bars.len(),
                            terminal.chart.symbol,
                            terminal.chart.timeframe
                        );
                        // Debug: Show first and last bar prices with timestamps
                        if !terminal.chart.bars.is_empty() {
                            let first = &terminal.chart.bars[0];
                            let last = &terminal.chart.bars[terminal.chart.bars.len() - 1];
                            println!(
                                "  First bar: O:{:.2} H:{:.2} L:{:.2} C:{:.2} ({})",
                                first.open,
                                first.high,
                                first.low,
                                first.close,
                                first.timestamp.format("%Y-%m-%d %H:%M")
                            );
                            println!(
                                "  Last bar:  O:{:.2} H:{:.2} L:{:.2} C:{:.2} ({})",
                                last.open,
                                last.high,
                                last.low,
                                last.close,
                                last.timestamp.format("%Y-%m-%d %H:%M")
                            );
                        }
                    }
                    Err(error) => {
                        terminal.chart.error = Some(error.clone());
                        terminal.chart.bars = generate_mock_data();
                        eprintln!("✗ Error fetching bars: {}. Using mock data.", error);
                    }
                }
                terminal.chart.loading = false;
                cx.notify();
            });
        })
        .detach();
    }

    fn render_candlesticks(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        if self.chart.bars.is_empty() {
            let message = if self.chart.loading {
                "Loading data from Alpaca Markets...".to_string()
            } else if let Some(ref error) = self.chart.error {
                error.clone()
            } else {
                "No data available.".to_string()
            };

            return div()
                .grid()
                .items_center()
                .justify_center()
                .size_full()
                .child(div().text_color(rgb(0x808080)).child(message));
        }

        // Calculate visible range of bars (windowing for scrolling)
        let bars_per_screen = self.chart.bars_per_screen;
        // Clamp start_index to valid range
        let start_index =
            (self.chart.chart_scroll_offset as usize).min(self.chart.bars.len().saturating_sub(1));
        let end_index = (start_index + bars_per_screen).min(self.chart.bars.len());
        // Ensure we don't have an empty range
        let start_index = if end_index > start_index {
            start_index
        } else {
            0
        };
        let visible_bars = &self.chart.bars[start_index..end_index];

        // Calculate price range for visible bars only
        let max_price = visible_bars
            .iter()
            .map(|b| b.close)
            .fold(f64::NEG_INFINITY, f64::max);
        let min_price = visible_bars
            .iter()
            .map(|b| b.close)
            .fold(f64::INFINITY, f64::min);

        let price_range = max_price - min_price;
        let price_padding = price_range * 0.1;
        let adjusted_max = max_price + price_padding;
        let adjusted_min = min_price - price_padding;
        let adjusted_range = adjusted_max - adjusted_min;

        // Calculate bar width based on visible bars with padding
        let padding_left_percent = 5.0; // 5% left padding
        let padding_right_percent = 5.0; // 5% right padding
        let usable_width_percent = 100.0 - padding_left_percent - padding_right_percent;

        let visible_bar_count = visible_bars.len() as f32;
        let bar_spacing_ratio = 0.2; // 20% spacing between bars
        let bar_width_percent =
            (usable_width_percent / visible_bar_count) * (1.0 - bar_spacing_ratio);
        let total_bar_width_percent = usable_width_percent / visible_bar_count;

        div()
            .flex()
            .flex_col()
            .gap_4()
            .size_full()
            .child(
                // Chart container - expands to fill available space
                div()
                    .id("chart-container")
                    .relative()
                    .flex_1()
                    .w_full()
                    .bg(rgb(0x1a1a1a))
                    .border_2()
                    .border_color(rgb(0x404040))
                    // Inner div with relative positioning for accurate mouse tracking
                    .child(
                        div()
                            .relative()
                            .size_full()
                            .overflow_hidden()
                            .on_mouse_move(cx.listener(
                                |this, event: &gpui::MouseMoveEvent, window, cx| {
                                    // CALIBRATION GUIDE for offset_y:
                                    // 1. Hover at the VERY TOP of the chart (where price is highest)
                                    // 2. If crosshair price is HIGHER than expected: INCREASE offset_y
                                    // 3. If crosshair price is LOWER than expected: DECREASE offset_y
                                    let offset_x = px(66.0);
                                    let offset_y = px(212.0); // Adjust this if top of chart is wrong

                                    let relative_x = event.position.x - offset_x;
                                    let relative_y = event.position.y - offset_y;

                                    this.chart.mouse_position = Some(gpui::Point {
                                        x: relative_x,
                                        y: relative_y,
                                    });

                                    // Calculate chart bounds from window size
                                    let window_bounds = window.bounds();
                                    let window_width: f32 = window_bounds.size.width.into();
                                    let window_height: f32 = window_bounds.size.height.into();

                                    // Chart width calculation
                                    let chart_width = window_width * 0.875 - 100.0;

                                    // FIXED-PIXEL APPROACH: Chart height = window height - all fixed UI elements
                                    // This works regardless of window size because we subtract absolute pixels
                                    //
                                    // CALIBRATION: Adjust bottom_offset if prices don't match grid
                                    // - If crosshair shows LOWER price than grid: INCREASE bottom_offset
                                    // - If crosshair shows HIGHER price than grid: DECREASE bottom_offset
                                    //
                                    // Components below the chart (approximate values):
                                    // - Scroll controls: ~50px
                                    // - Gap before footer: ~24px
                                    // - Footer: ~280px
                                    // - Window bottom padding: ~40px
                                    let bottom_offset = 414.0; // Tune this value

                                    let offset_y_f32: f32 = offset_y.into();
                                    let chart_height = window_height - offset_y_f32 - bottom_offset;

                                    // Debug: Print calibration info (comment out after calibration)
                                    println!("Window H: {:.0}px, Chart H: {:.0}px (= {:.0} - {:.0} - {:.0}), Mouse Y: {:.0}px",
                                             window_height, chart_height, window_height, offset_y_f32, bottom_offset, relative_y);

                                    this.chart.chart_bounds = Some((chart_width, chart_height));
                                    this.chart.show_crosshair = true;
                                    cx.notify();
                                },
                            ))
                            .on_scroll_wheel(cx.listener(
                                |this, event: &gpui::ScrollWheelEvent, _window, cx| {
                                    let pixel_delta = event.delta.pixel_delta(px(1.0));
                                    let scroll_amount: f32 = pixel_delta.y.into();

                                    // Check if Ctrl is pressed for zoom
                                    if event.modifiers.control {
                                        // Zoom: adjust bars_per_screen
                                        let zoom_amount = (scroll_amount * 2.0) as i32;

                                        if zoom_amount > 0 {
                                            // Zoom out (show more bars)
                                            this.chart.bars_per_screen = (this.chart.bars_per_screen
                                                + zoom_amount as usize)
                                                .min(this.chart.bars.len());
                                        } else {
                                            // Zoom in (show fewer bars)
                                            this.chart.bars_per_screen =
                                                (this.chart.bars_per_screen as i32 + zoom_amount).max(10)
                                                    as usize;
                                        }

                                        // Adjust scroll offset to keep it in bounds
                                        let max_offset =
                                            this.chart.bars.len().saturating_sub(this.chart.bars_per_screen)
                                                as f32;
                                        this.chart.chart_scroll_offset =
                                            this.chart.chart_scroll_offset.min(max_offset);
                                    } else {
                                        // Normal scroll: move through bars
                                        let max_offset =
                                            this.chart.bars.len().saturating_sub(this.chart.bars_per_screen)
                                                as f32;
                                        let scroll_amount = scroll_amount * 0.5; // Adjust sensitivity

                                        if scroll_amount > 0.0 {
                                            // Scroll forward (show older bars)
                                            this.chart.chart_scroll_offset = (this.chart.chart_scroll_offset
                                                + scroll_amount)
                                                .min(max_offset);
                                        } else {
                                            // Scroll backward (show newer bars)
                                            this.chart.chart_scroll_offset =
                                                (this.chart.chart_scroll_offset + scroll_amount).max(0.0);
                                        }
                                    }

                                    cx.notify();
                                },
                            ))
                            // Price grid lines with round values (adaptive to zoom level)
                            .children({
                                // Adjust grid line count based on zoom level
                                let grid_count = if self.chart.bars_per_screen <= 20 {
                                    12 // Very zoomed in - show many grid lines
                                } else if self.chart.bars_per_screen <= 50 {
                                    10 // Moderately zoomed in
                                } else if self.chart.bars_per_screen <= 100 {
                                    8 // Default zoom
                                } else if self.chart.bars_per_screen <= 200 {
                                    6 // Zoomed out
                                } else if self.chart.bars_per_screen <= 500 {
                                    5 // More zoomed out
                                } else {
                                    4 // Very zoomed out - show fewer grid lines
                                };

                                let grid_values = chart::calculate_round_grid_values(
                                    adjusted_min,
                                    adjusted_max,
                                    grid_count,
                                );
                                grid_values.into_iter().map(|price| {
                                    // Calculate Y position as percentage
                                    let y_percent =
                                        ((adjusted_max - price) / adjusted_range) as f32 * 100.0;

                                    div()
                                        .absolute()
                                        .left_0()
                                        .top(gpui::relative(y_percent / 100.0))
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
                                })
                            })
                            // Candlestick wicks
                            .children(visible_bars.iter().enumerate().map(|(i, bar)| {
                                // Calculate positions as percentages with padding
                                let x_percent =
                                    padding_left_percent + i as f32 * total_bar_width_percent;

                                // Calculate Y positions as percentages with padding
                                let padding_top_percent = 5.0;
                                let padding_bottom_percent = 5.0;
                                let usable_height_percent =
                                    100.0 - padding_top_percent - padding_bottom_percent;

                                let high_y_percent = padding_top_percent
                                    + ((adjusted_max - bar.high) / adjusted_range) as f32
                                        * usable_height_percent;
                                let low_y_percent = padding_top_percent
                                    + ((adjusted_max - bar.low) / adjusted_range) as f32
                                        * usable_height_percent;

                                let wick_height_percent = low_y_percent - high_y_percent;

                                // Determine if bullish or bearish
                                let is_bullish = bar.close >= bar.open;

                                // Check if this is the most recent bar (live updating)
                                let is_latest_bar = i == visible_bars.len() - 1 &&
                                    end_index == self.chart.bars.len();

                                let color = if is_bullish {
                                    rgb(0x00cc66)
                                } else {
                                    rgb(0xff4444)
                                };

                                // High-Low wick (thin line)
                                div()
                                    .absolute()
                                    .left(gpui::relative(
                                        (x_percent + bar_width_percent / 2.0) / 100.0,
                                    ))
                                    .top(gpui::relative(high_y_percent / 100.0))
                                    .w(if is_latest_bar { px(2.0) } else { px(1.0) })
                                    .h(gpui::relative(wick_height_percent / 100.0))
                                    .bg(color)
                            }))
                            // Candlestick bodies
                            .children(visible_bars.iter().enumerate().map(|(i, bar)| {
                                // Calculate positions as percentages with padding
                                let x_percent =
                                    padding_left_percent + i as f32 * total_bar_width_percent;

                                // Calculate Y positions as percentages with padding
                                let padding_top_percent = 5.0;
                                let padding_bottom_percent = 5.0;
                                let usable_height_percent =
                                    100.0 - padding_top_percent - padding_bottom_percent;

                                let open_y_percent = padding_top_percent
                                    + ((adjusted_max - bar.open) / adjusted_range) as f32
                                        * usable_height_percent;
                                let close_y_percent = padding_top_percent
                                    + ((adjusted_max - bar.close) / adjusted_range) as f32
                                        * usable_height_percent;

                                let body_top_percent = open_y_percent.min(close_y_percent);
                                let body_height_percent =
                                    (open_y_percent - close_y_percent).abs().max(0.1);

                                // Determine if bullish or bearish
                                let is_bullish = bar.close >= bar.open;

                                // Check if this is the most recent bar (live updating)
                                let is_latest_bar = i == visible_bars.len() - 1 &&
                                    end_index == self.chart.bars.len();

                                let (color, fill_color) = if is_bullish {
                                    (rgb(0x00cc66), rgb(0x00cc66))
                                } else {
                                    (rgb(0xff4444), rgb(0xff4444))
                                };

                                // Open-Close body (thicker rectangle)
                                let mut body_div = div()
                                    .absolute()
                                    .left(gpui::relative(x_percent / 100.0))
                                    .top(gpui::relative(body_top_percent / 100.0))
                                    .w(gpui::relative(bar_width_percent / 100.0))
                                    .h(gpui::relative(body_height_percent / 100.0))
                                    .bg(fill_color);

                                // Add thicker border and glow effect for the latest bar
                                if is_latest_bar {
                                    body_div = body_div
                                        .border_2()
                                        .border_color(color)
                                        .shadow_lg();
                                } else {
                                    body_div = body_div
                                        .border_1()
                                        .border_color(color);
                                }

                                body_div
                            }))
                            // Crosshair overlay
                            .children(if self.chart.show_crosshair && self.chart.mouse_position.is_some() {
                                let mouse_pos = self.chart.mouse_position.unwrap();

                                // Calculate price from mouse Y position
                                // Grid lines use full height (0-100%) without padding
                                let mouse_y_f32: f32 = mouse_pos.y.into();
                                let chart_height =
                                    self.chart.chart_bounds.map(|(_, h)| h).unwrap_or(400.0);

                                // Account for 2px border on chart container
                                let border_offset = 2.0;
                                let adjusted_mouse_y = mouse_y_f32 - border_offset;
                                let adjusted_chart_height = chart_height - (border_offset * 2.0);

                                let y_percent = (adjusted_mouse_y / adjusted_chart_height) * 100.0;

                                // Convert Y position to price (matches grid line calculation)
                                // Grid formula: y_percent = ((adjusted_max - price) / adjusted_range) * 100.0
                                // Inverse: price = adjusted_max - (y_percent / 100.0 * adjusted_range)
                                let price_at_cursor =
                                    adjusted_max - ((y_percent / 100.0) as f64 * adjusted_range);

                                // Debug: Print price calculation for calibration
                                println!("Y%%: {:.1}, Price: ${:.2}, Range: ${:.2}-${:.2}",
                                         y_percent, price_at_cursor, adjusted_min, adjusted_max);
                                println!(">>> If crosshair shows LOWER than grid: INCREASE bottom_offset (line 879)");
                                println!(">>> If crosshair shows HIGHER than grid: DECREASE bottom_offset (line 879)");
                                println!(">>> Current bottom_offset: 394.0 - Adjust by 5-10px increments");

                                // Calculate bar index from mouse X position
                                let mouse_x_f32: f32 = mouse_pos.x.into();
                                let chart_width =
                                    self.chart.chart_bounds.map(|(w, _)| w).unwrap_or(800.0);
                                let x_percent = (mouse_x_f32 / chart_width) * 100.0;

                                let padding_left_percent = 5.0;
                                let usable_width_percent = 100.0 - padding_left_percent - 5.0;
                                let bar_index = ((x_percent - padding_left_percent)
                                    / usable_width_percent
                                    * visible_bar_count)
                                    as usize;

                                // Get the timestamp if valid bar index
                                let timestamp_opt = if bar_index < visible_bars.len() {
                                    Some(visible_bars[bar_index].timestamp)
                                } else {
                                    None
                                };

                                let mut elements = vec![
                                    // Vertical crosshair line
                                    div()
                                        .absolute()
                                        .left(mouse_pos.x)
                                        .top(px(0.0))
                                        .w(px(1.0))
                                        .h(gpui::relative(1.0))
                                        .bg(gpui::rgba(0xFFFFFF40))
                                        .into_any_element(),
                                    // Horizontal crosshair line
                                    div()
                                        .absolute()
                                        .left(px(0.0))
                                        .top(mouse_pos.y)
                                        .w(gpui::relative(1.0))
                                        .h(px(1.0))
                                        .bg(gpui::rgba(0xFFFFFF40))
                                        .into_any_element(),
                                ];

                                // Price label on Y-axis (right side)
                                // Always show price label for calibration (removed bounds check)
                                elements.push(
                                    div()
                                        .absolute()
                                        .right(px(5.0))
                                        .top(mouse_pos.y - px(10.0))
                                        .px_2()
                                        .py_1()
                                        .bg(rgb(0x1f6feb))
                                        .border_1()
                                        .border_color(rgb(0x388bfd))
                                        .rounded_sm()
                                        .text_xs()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(rgb(0xffffff))
                                        .child(format!("${:.2}", price_at_cursor))
                                        .into_any_element(),
                                );

                                // Timestamp label on X-axis (bottom)
                                if let Some(timestamp) = timestamp_opt {
                                    // Format timestamp for display (MM-DD HH:MM)
                                    let display_time = timestamp.format("%m-%d %H:%M").to_string();

                                    elements.push(
                                        div()
                                            .absolute()
                                            .left(mouse_pos.x - px(40.0))
                                            .bottom(px(5.0))
                                            .px_2()
                                            .py_1()
                                            .bg(rgb(0x1f6feb))
                                            .border_1()
                                            .border_color(rgb(0x388bfd))
                                            .rounded_sm()
                                            .text_xs()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(rgb(0xffffff))
                                            .child(display_time)
                                            .into_any_element(),
                                    );
                                }

                                elements
                            } else {
                                vec![]
                            }),
                    ),
            )
            .child(
                // Scroll controls
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .items_center()
                    .justify_center()
                    .p_2()
                    .on_mouse_move(cx.listener(|this, _event, _window, cx| {
                        // Hide crosshair when mouse is over scroll controls
                        this.chart.show_crosshair = false;
                        cx.notify();
                    }))
                    .child(
                        div()
                            .px_3()
                            .py_1()
                            .bg(rgb(0x2a2a2a))
                            .border_1()
                            .border_color(rgb(0x404040))
                            .rounded_md()
                            .cursor_pointer()
                            .hover(|style| style.bg(rgb(0x3a3a3a)))
                            .on_mouse_down(
                                gpui::MouseButton::Left,
                                cx.listener(|this, _event: &gpui::MouseDownEvent, _window, cx| {
                                    if this.chart.chart_scroll_offset > 0.0 {
                                        this.chart.chart_scroll_offset =
                                            (this.chart.chart_scroll_offset - 50.0).max(0.0);
                                        cx.notify();
                                    }
                                }),
                            )
                            .child("← Previous 50"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_2()
                            .items_center()
                            .text_sm()
                            .text_color(rgb(0x808080))
                            .child(format!(
                                "Showing bars {}-{} of {} | Zoom: {} bars",
                                start_index + 1,
                                end_index,
                                self.chart.bars.len(),
                                self.chart.bars_per_screen
                            ))
                            .when(end_index == self.chart.bars.len() && self.chart.market_data_connected, |this| {
                                this.child(
                                    div()
                                        .px_2()
                                        .py_0p5()
                                        .bg(rgb(0x238636))
                                        .rounded_sm()
                                        .text_xs()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(rgb(0xffffff))
                                        .child("● LIVE")
                                )
                            })
                    )
                    .child(
                        div()
                            .px_3()
                            .py_1()
                            .bg(rgb(0x2a2a2a))
                            .border_1()
                            .border_color(rgb(0x404040))
                            .rounded_md()
                            .cursor_pointer()
                            .hover(|style| style.bg(rgb(0x3a3a3a)))
                            .on_mouse_down(
                                gpui::MouseButton::Left,
                                cx.listener(|this, _event: &gpui::MouseDownEvent, _window, cx| {
                                    let max_offset =
                                        this.chart.bars.len().saturating_sub(this.chart.bars_per_screen) as f32;
                                    if this.chart.chart_scroll_offset < max_offset {
                                        this.chart.chart_scroll_offset =
                                            (this.chart.chart_scroll_offset + 50.0).min(max_offset);
                                        cx.notify();
                                    }
                                }),
                            )
                            .child("Next 50 →"),
                    )
                    .child(
                        div()
                            .px_3()
                            .py_1()
                            .bg(rgb(0x1f6feb))
                            .border_1()
                            .border_color(rgb(0x404040))
                            .rounded_md()
                            .cursor_pointer()
                            .hover(|style| style.bg(rgb(0x2a7ffc)))
                            .on_mouse_down(
                                gpui::MouseButton::Left,
                                cx.listener(|this, _event: &gpui::MouseDownEvent, _window, cx| {
                                    // Show most recent bars
                                    this.chart.chart_scroll_offset =
                                        this.chart.bars.len().saturating_sub(this.chart.bars_per_screen) as f32;
                                    cx.notify();
                                }),
                            )
                            .child("Show Latest →→"),
                    ),
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
                    .child(div().child(format!("Bars: {}", self.chart.bars.len())))
                    .when_some(self.chart.bars.last(), |this, last_bar| {
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

impl Render for TradingTerminal {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let timeframe_display = match self.chart.timeframe.as_str() {
            "1Min" => "1 Minute",
            "5Min" => "5 Minutes",
            "15Min" => "15 Minutes",
            "1Hour" => "1 Hour",
            "1Day" => "Daily",
            "1Week" => "Weekly",
            "1Month" => "Monthly",
            _ => &self.chart.timeframe,
        };

        div()
            .grid()
            .grid_cols(8)
            .grid_rows(1)
            .bg(rgb(0x0d1117))
            .size_full()
            .min_w(px(1024.0))
            .gap_4()
            .child(
                // Main content area (left column) - flex layout for header/chart/footer
                div()
                    .col_span(7)
                    .flex()
                    .flex_col()
                    .p_8()
                    .gap_6()
                    .track_focus(&self.focus_handle)
                    .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, _, cx| {
                        // Handle symbol input
                        if this.chart.input_focused {
                            let key = event.keystroke.key.as_str();

                            if key == "enter" {
                                this.submit_symbol(cx);
                            } else if key == "backspace" {
                                this.handle_backspace(cx);
                            } else if key == "escape" {
                                this.chart.input_focused = false;
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

                        // Handle bar limit input
                        if this.chart.bar_limit_focused {
                            let key = event.keystroke.key.as_str();

                            if key == "enter" {
                                this.fetch_bars(cx);
                            } else if key == "backspace" {
                                this.chart.bar_limit.pop();
                                cx.notify();
                            } else if key == "escape" {
                                this.chart.bar_limit_focused = false;
                                cx.notify();
                            } else if let Some(key_char) = &event.keystroke.key_char {
                                if key_char.len() == 1 && key_char.chars().all(|c| c.is_numeric()) {
                                    this.chart.bar_limit.push_str(key_char);
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
                            .flex_shrink_0()
                            .items_center()
                            .justify_between()
                            .on_mouse_move(cx.listener(|this, _event, _window, cx| {
                                // Hide crosshair when mouse is over header
                                this.chart.show_crosshair = false;
                                cx.notify();
                            }))
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
                                                            .bg(if self.chart.input_focused {
                                                                rgb(0x1f2937)
                                                            } else {
                                                                rgb(0x161b22)
                                                            })
                                                            .border_1()
                                                            .border_color(if self.chart.input_focused {
                                                                rgb(0x1f6feb)
                                                            } else {
                                                                rgb(0x30363d)
                                                            })
                                                            .rounded_lg()
                                                            .text_color(rgb(0xffffff))
                                                            .min_w(px(120.0))
                                                            .cursor_text()
                                                            .child(if self.chart.input_focused {
                                                                format!("{}|", self.chart.symbol_input)
                                                            } else if self.chart.symbol_input.is_empty() {
                                                                "Enter symbol...".to_string()
                                                            } else {
                                                                self.chart.symbol_input.clone()
                                                            })
                                                            .on_click(cx.listener(
                                                                |this, _, _window, cx| {
                                                                    this.chart.input_focused = true;
                                                                    _window
                                                                        .focus(&this.focus_handle);
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
                                                            .on_click(cx.listener(
                                                                |this, _, _, cx| {
                                                                    this.submit_symbol(cx);
                                                                },
                                                            )),
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
                                                    .child(
                                                        self.render_timeframe_button(
                                                            "1Min", "1m", cx,
                                                        ),
                                                    )
                                                    .child(
                                                        self.render_timeframe_button(
                                                            "5Min", "5m", cx,
                                                        ),
                                                    )
                                                    .child(self.render_timeframe_button(
                                                        "15Min", "15m", cx,
                                                    ))
                                                    .child(
                                                        self.render_timeframe_button(
                                                            "1Hour", "1h", cx,
                                                        ),
                                                    )
                                                    .child(
                                                        self.render_timeframe_button(
                                                            "1Day", "1D", cx,
                                                        ),
                                                    )
                                                    .child(
                                                        self.render_timeframe_button(
                                                            "1Week", "1W", cx,
                                                        ),
                                                    )
                                                    .child(self.render_timeframe_button(
                                                        "1Month", "1M", cx,
                                                    )),
                                            ),
                                    )
                                    .child(
                                        // Bar limit input
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(rgb(0xffffff))
                                                    .child("Bars:"),
                                            )
                                            .child(
                                                div()
                                                    .id("bar-limit-input")
                                                    .px_4()
                                                    .py_2()
                                                    .bg(if self.chart.bar_limit_focused {
                                                        rgb(0x1f2937)
                                                    } else {
                                                        rgb(0x161b22)
                                                    })
                                                    .border_1()
                                                    .border_color(if self.chart.bar_limit_focused {
                                                        rgb(0x1f6feb)
                                                    } else {
                                                        rgb(0x30363d)
                                                    })
                                                    .rounded_lg()
                                                    .text_color(rgb(0xffffff))
                                                    .min_w(px(80.0))
                                                    .cursor_text()
                                                    .child(if self.chart.bar_limit_focused {
                                                        format!("{}|", self.chart.bar_limit)
                                                    } else if self.chart.bar_limit.is_empty() {
                                                        "100".to_string()
                                                    } else {
                                                        self.chart.bar_limit.clone()
                                                    })
                                                    .on_click(cx.listener(
                                                        |this, _, _window, cx| {
                                                            this.chart.bar_limit_focused = true;
                                                            this.chart.input_focused = false;
                                                            this.quantity_focused = false;
                                                            this.price_focused = false;
                                                            _window.focus(&this.focus_handle);
                                                            cx.notify();
                                                        },
                                                    )),
                                            ),
                                    ),
                            )
                            .child(
                                // Title section
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_2xl()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(rgb(0xffffff))
                                            .child(format!("{} Stock Chart", self.chart.symbol)),
                                    )
                                    .child(div().text_sm().text_color(rgb(0x808080)).child(
                                        format!(
                                            "{} candlestick chart powered by Alpaca Markets",
                                            timeframe_display
                                        ),
                                    )),
                            )
                            .child(
                                // Status and controls section
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_3()
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
                                            .child(
                                                div().text_sm().text_color(rgb(0xffffff)).child(
                                                    if self.stream_connected {
                                                        "🟢 Live Updates"
                                                    } else {
                                                        "⭕ Disconnected"
                                                    },
                                                ),
                                            ),
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
                                            .bg(if self.chart.market_data_connected {
                                                rgb(0x1f6feb)
                                            } else {
                                                rgb(0x6e7681)
                                            })
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_col()
                                                    .gap_1()
                                                    .child(
                                                        div().text_sm().font_weight(FontWeight::SEMIBOLD).text_color(rgb(0xffffff)).child(
                                                            if self.chart.market_data_connected {
                                                                "📊 Market Data Stream"
                                                            } else {
                                                                "📊 No Market Data"
                                                            }
                                                        )
                                                    )
                                                    .when(self.chart.market_data_connected && self.chart.last_bar_symbol.is_some(), |this| {
                                                        this.child(
                                                            div()
                                                                .flex()
                                                                .flex_col()
                                                                .gap_1()
                                                                .text_xs()
                                                                .text_color(rgb(0xcccccc))
                                                                .child(
                                                                    div().child(format!(
                                                                        "Symbol: {} | Time: {}",
                                                                        self.chart.last_bar_symbol.as_ref().unwrap(),
                                                                        self.chart.last_bar_time.as_ref().map(|t| {
                                                                            if t.len() >= 19 {
                                                                                &t[11..19] // HH:MM:SS
                                                                            } else {
                                                                                t.as_str()
                                                                            }
                                                                        }).unwrap_or("--:--:--")
                                                                    ))
                                                                )
                                                                .child(
                                                                    div().child(format!(
                                                                        "O: {} | H: {} | L: {} | C: {}",
                                                                        self.chart.last_bar_open.as_ref().unwrap_or(&"--".to_string()),
                                                                        self.chart.last_bar_high.as_ref().unwrap_or(&"--".to_string()),
                                                                        self.chart.last_bar_low.as_ref().unwrap_or(&"--".to_string()),
                                                                        self.chart.last_bar_close.as_ref().unwrap_or(&"--".to_string()),
                                                                    ))
                                                                )
                                                                .child(
                                                                    div().child(format!(
                                                                        "Volume: {}",
                                                                        self.chart.last_bar_volume.as_ref().unwrap_or(&"--".to_string()),
                                                                    ))
                                                                )
                                                        )
                                                    }),
                                            ),
                                    ),
                            )
                            .child(
                                // Refresh button
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
                                    .child(if self.chart.loading {
                                        "⟳ Loading..."
                                    } else {
                                        "↻ Refresh Data"
                                    })
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.fetch_bars(cx);
                                    })),
                            ),
                    )
                    .child(
                        // Spacer div between header and chart to catch mouse events in the gap
                        div().h(px(24.0)).w_full().on_mouse_move(cx.listener(
                            |this, _event, _window, cx| {
                                this.chart.show_crosshair = false;
                                cx.notify();
                            },
                        )),
                    )
                    .child(
                        // Chart area wrapper with side padding to catch mouse events
                        div()
                            .flex_1()
                            .flex()
                            .flex_row()
                            .min_h(px(400.0))
                            .child(
                                // Left padding area to catch mouse events
                                div().w(px(32.0)).h_full().on_mouse_move(cx.listener(
                                    |this, _event, _window, cx| {
                                        this.chart.show_crosshair = false;
                                        cx.notify();
                                    },
                                )),
                            )
                            .child(
                                // Actual chart
                                div()
                                    .flex_1()
                                    .grid()
                                    .items_center()
                                    .justify_center()
                                    .child(self.render_candlesticks(cx)),
                            )
                            .child(
                                // Right padding area to catch mouse events
                                div().w(px(32.0)).h_full().on_mouse_move(cx.listener(
                                    |this, _event, _window, cx| {
                                        this.chart.show_crosshair = false;
                                        cx.notify();
                                    },
                                )),
                            ),
                    )
                    .child(
                        // Spacer div between chart and footer to catch mouse events in the gap
                        div().h(px(24.0)).w_full().on_mouse_move(cx.listener(
                            |this, _event, _window, cx| {
                                this.chart.show_crosshair = false;
                                cx.notify();
                            },
                        )),
                    )
                    .child(
                        // Tabbed Footer
                        div()
                            .flex_shrink_0()
                            .grid()
                            .grid_cols(1)
                            .gap_3()
                            .p_4()
                            .bg(rgb(0x161b22))
                            .rounded_lg()
                            .border_1()
                            .border_color(rgb(0x30363d))
                            .on_mouse_move(cx.listener(|this, _event, _window, cx| {
                                // Hide crosshair when mouse is over footer
                                this.chart.show_crosshair = false;
                                cx.notify();
                            }))
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
            ) // Close main content .child()
            .child(
                // Right sidebar - Order form
                div()
                    .col_span(1)
                    .bg(rgb(0x161b22))
                    .border_l_1()
                    .border_color(rgb(0x30363d))
                    .p_6()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .on_mouse_move(cx.listener(|this, _event, _window, cx| {
                        // Hide crosshair when mouse is over sidebar
                        this.chart.show_crosshair = false;
                        cx.notify();
                    }))
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
                                    .child(self.chart.symbol.clone()),
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
                                        this.chart.input_focused = false;
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
                                    self.chart.symbol
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

impl TradingTerminal {
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
                .grid()
                .items_center()
                .justify_center()
                .p_6()
                .text_color(rgb(0x8b949e))
                .child("Loading positions...");
        }

        if self.positions.is_empty() {
            return div()
                .grid()
                .items_center()
                .justify_center()
                .p_6()
                .text_color(rgb(0x8b949e))
                .child("No active positions");
        }

        div()
            .grid()
            .grid_cols(1)
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
                .grid()
                .items_center()
                .justify_center()
                .p_6()
                .text_color(rgb(0x8b949e))
                .child("Loading orders...");
        }

        if self.orders.is_empty() {
            return div()
                .grid()
                .items_center()
                .justify_center()
                .p_6()
                .text_color(rgb(0x8b949e))
                .child("No active orders");
        }

        div()
            .grid()
            .grid_cols(1)
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
                        this.chart.input_focused = false;
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
        let is_selected = self.chart.timeframe == timeframe;
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
                this.chart.timeframe = timeframe_owned.clone();
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
// Uses split-adjusted data with sort=desc to get most recent bars
fn fetch_bars_sync(symbol: &str, timeframe: &str, user_limit: u32) -> Result<Vec<Bar>, String> {
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

        // Calculate time range - use generous lookback since we'll sort descending
        let end_time = Utc::now();
        let start_time = match timeframe {
            // Intraday: calculate days needed based on bars/day during market hours
            "1Min" => end_time - Duration::days(((user_limit as i64) / 390).max(1) + 2),
            "5Min" => end_time - Duration::days(((user_limit as i64) / 78).max(1) + 2),
            "15Min" => end_time - Duration::days(((user_limit as i64) / 26).max(1) + 2),
            "1Hour" => end_time - Duration::days(((user_limit as i64) / 6).max(1) + 5),
            // Daily+: straightforward calculation with buffer for weekends/holidays
            "1Day" => end_time - Duration::days((user_limit as i64 * 3) / 2),
            "1Week" => end_time - Duration::days((user_limit as i64 * 7) + 14),
            "1Month" => end_time - Duration::days((user_limit as i64 * 30) + 60),
            _ => end_time - Duration::days((user_limit as i64 * 3) / 2),
        };

        // Use Sort::Desc to get most recent bars first, with split adjustment
        // The API will return the most recent N bars when sorted descending
        let result = client
            .get_bars(
                symbol,
                timeframe,
                Some(start_time),
                Some(end_time),
                Some(user_limit),
                Some(Sort::Desc),        // Sort descending to get most recent bars
                Some(Adjustment::Split), // Adjust for stock splits
            )
            .await;

        match result {
            Ok(bars_response) => {
                // Reverse bars to chronological order (oldest first) for chart rendering
                let mut bars = bars_response.bars;
                bars.reverse();
                Ok(bars)
            }
            Err(e) => Err(format!("Error fetching data: {:?}", e)),
        }
    })
}

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

        cx.open_window(WindowOptions::default(), |_, cx| {
            cx.new(TradingTerminal::new)
        })
        .unwrap();
    });
}
