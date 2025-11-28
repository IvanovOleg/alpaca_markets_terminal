use alpaca_markets::{AlpacaConfig, Bar, MarketDataClient};
use chrono::{Duration, Utc};
use gpui::{
    App, Application, Context, ElementId, FocusHandle, FontWeight, IntoElement, Render, Window,
    WindowOptions, actions, div, prelude::*, px, rgb,
};

actions!(app, [Quit, RefreshData]);

struct BarChart {
    symbol: String,
    symbol_input: String,
    timeframe: String,
    bars: Vec<Bar>,
    loading: bool,
    error: Option<String>,
    input_focused: bool,
    focus_handle: FocusHandle,
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
        };

        // Fetch data on startup
        chart.fetch_bars(cx);
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
                                    .left(px(x + actual_candle_width / 2.0 - 0.5))
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
            .flex_col()
            .bg(rgb(0x0d1117))
            .size_full()
            .p_8()
            .gap_6()
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, event: &gpui::KeyDownEvent, _, cx| {
                if !this.input_focused {
                    return;
                }

                let key = event.keystroke.key.as_str();

                if key == "enter" {
                    this.submit_symbol(cx);
                } else if key == "backspace" {
                    this.handle_backspace(cx);
                } else if key == "escape" {
                    this.input_focused = false;
                    cx.notify();
                } else if let Some(key_char) = &event.keystroke.key_char {
                    // Use key_char for actual character input (handles shift + letter for uppercase)
                    if key_char.len() == 1 && key_char.chars().all(|c| c.is_alphanumeric()) {
                        this.handle_input(key_char, cx);
                    }
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
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(rgb(0x808080))
                                    .child(format!("{} candlestick chart powered by Alpaca Markets", timeframe_display)),
                            ),
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
                                            .child(
                                                if self.input_focused {
                                                    format!("{}|", self.symbol_input)
                                                } else if self.symbol_input.is_empty() {
                                                    "Enter symbol...".to_string()
                                                } else {
                                                    self.symbol_input.clone()
                                                }
                                            )
                                            .on_click(cx.listener(|this, _, _window, cx| {
                                                this.input_focused = true;
                                                _window.focus(&this.focus_handle);
                                                cx.notify();
                                            })),
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
                                    .child(self.render_timeframe_button("1Month", "1M", cx)),
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
                // Footer with legend and instructions
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
                        div()
                            .text_sm()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(0xffffff))
                            .child("Legend & Instructions:"),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_8()
                            .text_xs()
                            .text_color(rgb(0x8b949e))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .w(px(16.0))
                                            .h(px(16.0))
                                            .bg(rgb(0x00cc66))
                                            .rounded_sm(),
                                    )
                                    .child("Green = Bullish (Close ≥ Open)"),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .w(px(16.0))
                                            .h(px(16.0))
                                            .bg(rgb(0xff4444))
                                            .rounded_sm(),
                                    )
                                    .child("Red = Bearish (Close < Open)"),
                            )
                            .child("Wicks show High/Low range")
                            .child("Body shows Open/Close range"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(0x8b949e))
                            .child("💡 Set APCA_API_KEY_ID and APCA_API_SECRET_KEY environment variables to fetch live data from Alpaca Markets."),
                    ),
            )
    }
}

impl BarChart {
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
