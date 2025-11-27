use gpui::{
    App, Application, Context, FontWeight, IntoElement, Render, Window, WindowOptions, actions,
    div, prelude::*, px, rgb,
};

actions!(app, [Quit]);

struct CandlestickChart {
    symbol: String,
    bars: Vec<Candlestick>,
}

#[derive(Clone, Debug)]
struct Candlestick {
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: u64,
}

impl Candlestick {
    fn is_bullish(&self) -> bool {
        self.close >= self.open
    }
}

impl CandlestickChart {
    fn new(_cx: &mut Context<Self>) -> Self {
        // Create some mock data for testing
        let mock_bars = vec![
            Candlestick {
                open: 150.0,
                high: 155.0,
                low: 148.0,
                close: 153.0,
                volume: 1000000,
            },
            Candlestick {
                open: 153.0,
                high: 158.0,
                low: 152.0,
                close: 156.0,
                volume: 1200000,
            },
            Candlestick {
                open: 156.0,
                high: 160.0,
                low: 154.0,
                close: 155.0,
                volume: 1100000,
            },
            Candlestick {
                open: 155.0,
                high: 157.0,
                low: 150.0,
                close: 151.0,
                volume: 1300000,
            },
            Candlestick {
                open: 151.0,
                high: 154.0,
                low: 149.0,
                close: 153.0,
                volume: 1050000,
            },
            Candlestick {
                open: 153.0,
                high: 159.0,
                low: 153.0,
                close: 158.0,
                volume: 1400000,
            },
            Candlestick {
                open: 158.0,
                high: 162.0,
                low: 157.0,
                close: 161.0,
                volume: 1500000,
            },
            Candlestick {
                open: 161.0,
                high: 165.0,
                low: 160.0,
                close: 163.0,
                volume: 1600000,
            },
            Candlestick {
                open: 163.0,
                high: 164.0,
                low: 159.0,
                close: 160.0,
                volume: 1250000,
            },
            Candlestick {
                open: 160.0,
                high: 162.0,
                low: 157.0,
                close: 159.0,
                volume: 1150000,
            },
            Candlestick {
                open: 159.0,
                high: 163.0,
                low: 158.0,
                close: 162.0,
                volume: 1350000,
            },
            Candlestick {
                open: 162.0,
                high: 168.0,
                low: 161.0,
                close: 167.0,
                volume: 1700000,
            },
            Candlestick {
                open: 167.0,
                high: 170.0,
                low: 165.0,
                close: 168.0,
                volume: 1800000,
            },
            Candlestick {
                open: 168.0,
                high: 172.0,
                low: 167.0,
                close: 171.0,
                volume: 1900000,
            },
            Candlestick {
                open: 171.0,
                high: 173.0,
                low: 168.0,
                close: 169.0,
                volume: 1450000,
            },
            Candlestick {
                open: 169.0,
                high: 171.0,
                low: 166.0,
                close: 167.0,
                volume: 1350000,
            },
            Candlestick {
                open: 167.0,
                high: 169.0,
                low: 164.0,
                close: 165.0,
                volume: 1400000,
            },
            Candlestick {
                open: 165.0,
                high: 167.0,
                low: 163.0,
                close: 166.0,
                volume: 1300000,
            },
            Candlestick {
                open: 166.0,
                high: 170.0,
                low: 165.0,
                close: 169.0,
                volume: 1550000,
            },
            Candlestick {
                open: 169.0,
                high: 175.0,
                low: 168.0,
                close: 174.0,
                volume: 2000000,
            },
        ];

        Self {
            symbol: "AAPL".to_string(),
            bars: mock_bars,
        }
    }

    fn render_candlesticks(&self) -> impl IntoElement {
        if self.bars.is_empty() {
            return div()
                .flex()
                .items_center()
                .justify_center()
                .size_full()
                .child(div().text_color(rgb(0x808080)).child("No data available."));
        }

        let chart_width = 1200.0_f32;
        let chart_height = 600.0_f32;
        let padding = 60.0_f32;

        // Calculate price range
        let max_price = self
            .bars
            .iter()
            .map(|c| c.high)
            .fold(f64::NEG_INFINITY, f64::max);
        let min_price = self
            .bars
            .iter()
            .map(|c| c.low)
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
                    .children(self.bars.iter().enumerate().map(|(i, candle)| {
                        let x = padding + i as f32 * candle_width;

                        // Calculate Y positions (inverted because canvas origin is top-left)
                        let high_y = padding
                            + ((adjusted_max - candle.high) / adjusted_range) as f32
                                * (chart_height - 2.0 * padding);
                        let low_y = padding
                            + ((adjusted_max - candle.low) / adjusted_range) as f32
                                * (chart_height - 2.0 * padding);
                        let open_y = padding
                            + ((adjusted_max - candle.open) / adjusted_range) as f32
                                * (chart_height - 2.0 * padding);
                        let close_y = padding
                            + ((adjusted_max - candle.close) / adjusted_range) as f32
                                * (chart_height - 2.0 * padding);

                        let body_top = open_y.min(close_y);
                        let body_height = (open_y - close_y).abs().max(1.0);

                        let (color, fill_color) = if candle.is_bullish() {
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
                        this.child(
                            div()
                                .text_color(if last_bar.is_bullish() {
                                    rgb(0x00cc66)
                                } else {
                                    rgb(0xff4444)
                                })
                                .child(format!("Last Close: ${:.2}", last_bar.close)),
                        )
                    }),
            )
    }
}

impl Render for CandlestickChart {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .bg(rgb(0x0d1117))
            .size_full()
            .p_8()
            .gap_6()
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
                                    .child("Candlestick chart demonstration"),
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
                                    .child("Green = Bullish (Close > Open)"),
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
                            .child("💡 This is a demonstration with mock data. To use live Alpaca Markets data, set APCA_API_KEY_ID and APCA_API_SECRET_KEY environment variables."),
                    ),
            )
    }
}

fn main() {
    Application::new().run(|cx: &mut App| {
        cx.activate(true);
        cx.on_action(|_: &Quit, cx| cx.quit());

        cx.open_window(WindowOptions::default(), |_, cx| {
            cx.new(CandlestickChart::new)
        })
        .unwrap();
    });
}
