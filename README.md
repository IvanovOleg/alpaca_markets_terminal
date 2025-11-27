# Alpaca Markets Terminal - Candlestick Chart

A beautiful stock trading chart application built with Rust and GPUI. Currently displays candlestick charts with mock data, with plans to integrate Alpaca Markets API for live market data.

![Candlestick Chart](https://img.shields.io/badge/Rust-1.70+-orange.svg)
![GPUI](https://img.shields.io/badge/GPUI-Latest-blue.svg)
![Alpaca Markets](https://img.shields.io/badge/Alpaca-Markets-green.svg)

## Features

✨ **Beautiful Candlestick Charts**
- Daily candlestick visualization with 20 sample bars
- Green candles for bullish days (close > open)
- Red candles for bearish days (close < open)
- High/Low wicks showing price range
- Price grid lines for easy reading

📊 **Data Features**
- Currently using mock data for demonstration
- Clean, professional chart rendering
- Real-time statistics display (High, Low, Range, etc.)
- Ready for Alpaca Markets API integration

🎨 **Modern UI**
- Dark theme optimized for long viewing sessions
- Clean, GitHub-inspired design
- Responsive layout
- Real-time statistics display

## Prerequisites

- Rust 1.70 or higher
- Windows SDK (for DirectX shader compilation)
- (Optional) Alpaca Markets account for future live data integration

## Installation

1. **Navigate to the project directory**
   ```bash
   cd C:\Users\oliva\projects\alpaca_markets_terminal
   ```

2. **Build the project**
   ```bash
   cargo build --release
   ```

3. **Run the application**
   ```bash
   cargo run --release
   ```

That's it! The application will launch with demo candlestick data.

## Usage

### Basic Usage

1. Launch the application:
   ```bash
   cargo run
   ```

2. The chart will display with 20 candlesticks of mock AAPL (Apple) stock data

3. Observe the beautiful candlestick visualization with price grid lines and statistics

### Understanding the Chart

**Candlestick Components:**
- **Body**: Rectangle showing open and close prices
  - Green body: Close price is higher than open (bullish)
  - Red body: Close price is lower than open (bearish)
- **Wicks**: Thin lines showing the high and low prices of the day
  - Upper wick: Distance from body top to daily high
  - Lower wick: Distance from body bottom to daily low

**Statistics Displayed:**
- High: Highest price in the displayed range
- Low: Lowest price in the displayed range
- Range: Price difference between high and low
- Bars: Number of candlesticks displayed
- Last Close: Most recent closing price

### Customizing the Chart

To change the symbol label or modify the mock data, edit `src/main.rs`:

```rust
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
        // Add more candlesticks...
    ];

    Self {
        symbol: "AAPL".to_string(), // Change to any symbol name
        bars: mock_bars,
    }
}
```

## Project Structure

```
alpaca_markets_terminal/
├── src/
│   └── main.rs           # Main application code
├── Cargo.toml            # Dependencies and project metadata
├── .env.example          # Example environment configuration
├── .env                  # Your actual API credentials (not committed)
└── README.md             # This file
```

## Dependencies

- **gpui**: Modern GPU-accelerated UI framework for stunning visuals
- **alpaca_markets**: Alpaca Markets API client library (for future integration)
- **tokio**: Async runtime (for future API calls)
- **chrono**: Date and time handling (for future timestamp support)

## Future: Live Data Integration

The application is ready to integrate with Alpaca Markets API for live data:

### Planned Features
- **Market Data API**: Fetch historical bars (OHLCV data)
- **IEX Feed**: Use the free IEX data feed (suitable for testing)
- **Multiple Timeframes**: Support for 1Min, 5Min, 1Hour, 1Day, etc.
- **Real-time Updates**: WebSocket support for live price updates
- **Multiple Symbols**: Display charts for any US stock

### Integration Steps
1. Set up environment variables with your API keys
2. Uncomment the async data fetching code
3. Replace mock data with live API calls
4. Add refresh button to reload data

## Troubleshooting

### Build fails with "Failed to find fxc.exe"

**Solution**: This is a Windows DirectX shader compiler issue. You need the Windows SDK installed:
1. Install Visual Studio 2019 or newer with "Desktop development with C++" workload
2. Make sure Windows 10 SDK is selected during installation
3. Alternatively, run the build from a "Developer Command Prompt for VS"

### Application doesn't start

**Solution**: 
1. Ensure you have Rust 1.70+ installed: `rustc --version`
2. Clean and rebuild: `cargo clean && cargo build`
3. Check all dependencies are available

### Window doesn't appear or crashes on startup

**Solution**:
1. Make sure your GPU drivers are up to date
2. Check that you have DirectX 12 support
3. Try running in debug mode: `cargo run` (without --release)

## Development

### Building from Source

```bash
# Debug build
cargo build

# Release build (optimized)
cargo build --release

# Run with logging
RUST_LOG=debug cargo run
```

### Adding New Features

The codebase is structured to be easily extensible:

1. **Add more mock data**: Extend the `mock_bars` vector in the `new()` function
2. **Add multiple charts**: Create multiple `CandlestickChart` instances
3. **Add indicators**: Extend the `render_candlesticks` method (RSI, MACD, etc.)
4. **Integrate live data**: Uncomment alpaca_markets imports and add async fetch functions
5. **Add interactivity**: Implement zoom, pan, or time range selection

## Resources

- [Alpaca Markets Documentation](https://alpaca.markets/docs/)
- [Alpaca Markets API Reference](https://alpaca.markets/docs/api-references/market-data-api/)
- [GPUI Framework](https://www.gpui.rs/)
- [Rust Programming Language](https://www.rust-lang.org/)

## Contributing

Contributions are welcome! Feel free to:

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Submit a pull request

## License

This project is provided as-is for educational purposes.

## Current Status

✅ **Working**
- Beautiful candlestick chart rendering
- Mock data display with 20 bars
- Price grid lines and statistics
- Clean, modern UI

🚧 **In Progress**
- Live Alpaca Markets API integration
- Multiple timeframe support
- Interactive refresh button
- Real-time WebSocket updates

## Disclaimer

This software is for educational and informational purposes only. It should not be considered financial advice. Always do your own research before making investment decisions.

## Acknowledgments

- Built with [GPUI](https://www.gpui.rs/) - A modern GPU-accelerated UI framework
- Powered by [Alpaca Markets](https://alpaca.markets/) - Commission-free trading API
- Inspired by professional trading terminals

---

Made with ❤️ and Rust