# Quick Start Guide

## Get Up and Running in 3 Steps

### Step 1: Build the Project

Open a terminal in the project directory and run:

```bash
cargo build --release
```

⏱️ This will take 3-5 minutes on first build as it compiles all dependencies.

### Step 2: Run the Application

```bash
cargo run --release
```

Or use the compiled binary directly:

```bash
target\release\alpaca_markets_terminal.exe
```

### Step 3: Enjoy!

The application will launch and display a beautiful candlestick chart with mock AAPL stock data.

## What You'll See

```
┌─────────────────────────────────────────────────────────────┐
│  AAPL Stock Chart                                           │
│  Candlestick chart demonstration                            │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  $173.00 ─────────────────────────────────────────────────  │
│         │                                            ┃┃      │
│  $168.00 ─────────┃┃─────────────────────────┃┃─────┃┃──    │
│         │    ┃┃   ┃┃    ┃┃    ┃┃    ┃┃    ┃┃ ┃┃ ┃┃  ┃┃      │
│  $163.00 ─────┃┃───┃┃────┃┃────┃┃────┃┃────┃┃─┃┃─┃┃──┃┃──    │
│         │ ┃┃ ┃┃   ┃┃ ┃┃ ┃┃ ┃┃ ┃┃ ┃┃ ┃┃ ┃┃ ┃┃ ┃┃ ┃┃  ┃┃      │
│  $158.00 ─┃┃─┃┃───┃┃─┃┃─┃┃─┃┃─┃┃─┃┃─┃┃─┃┃─┃┃─┃┃─┃┃──┃┃──    │
│         │ ┃┃ ┃┃      ┃┃ ┃┃ ┃┃ ┃┃ ┃┃ ┃┃ ┃┃ ┃┃ ┃┃ ┃┃           │
│  $153.00 ─┃┃─┃┃──────┃┃─┃┃─┃┃─┃┃─┃┃─┃┃─┃┃─┃┃─┃┃─┃┃───────    │
│         │ ┃┃                                                  │
│  $148.00 ─┃┃──────────────────────────────────────────────    │
│         └──────────────────────────────────────────────────  │
│                                                              │
│  High: $175.00  Low: $148.00  Range: $27.00  Bars: 20       │
│  Last Close: $174.00                                         │
├─────────────────────────────────────────────────────────────┤
│  Legend & Instructions:                                      │
│  🟢 Green = Bullish   🔴 Red = Bearish   Wicks show H/L     │
│  💡 This is demo data. See README for live data integration  │
└─────────────────────────────────────────────────────────────┘
```

## Understanding the Chart

### Candlestick Anatomy

```
    │  <- Upper Wick (High)
    │
  ┌───┐
  │   │ <- Body (Open to Close)
  └───┘
    │
    │  <- Lower Wick (Low)
```

- **Green Candle**: Price went up (Close > Open) - Bullish
- **Red Candle**: Price went down (Close < Open) - Bearish
- **Wick (thin line)**: Shows the full price range (High to Low)
- **Body (thick rectangle)**: Shows where price opened and closed

### Statistics Explained

- **High**: Highest price across all visible candles
- **Low**: Lowest price across all visible candles  
- **Range**: Difference between High and Low
- **Bars**: Number of candlesticks shown (20)
- **Last Close**: The closing price of the most recent candle

## Troubleshooting

### Build Fails

**Error: "Failed to find fxc.exe"**

This means you need the Windows SDK for shader compilation:

1. Install [Visual Studio 2019 or newer](https://visualstudio.microsoft.com/downloads/)
2. Choose "Desktop development with C++" workload
3. Ensure Windows 10 SDK is selected
4. Or run from "Developer Command Prompt for VS"

**Error: "linking with `link.exe` failed"**

Make sure you have the Microsoft C++ build tools installed.

### Application Won't Start

1. Update your GPU drivers
2. Ensure DirectX 12 is supported
3. Try debug mode: `cargo run` (slower but more compatible)

### Window Appears Blank

1. Check GPU compatibility (needs DirectX 12)
2. Update graphics drivers
3. Try running on integrated graphics if available

## Next Steps

### Want to Customize?

Edit `src/main.rs` to:
- Change the symbol name (line 173)
- Modify mock data (lines 30-170)
- Adjust colors (search for `rgb(0x...)`)
- Change chart size (lines 193-195)

### Want Live Data?

See `README.md` for instructions on integrating with Alpaca Markets API:
1. Get free API keys from [alpaca.markets](https://alpaca.markets)
2. Set environment variables
3. Uncomment the API integration code
4. Enjoy real-time stock data!

## Commands Reference

```bash
# Build (debug - fast compile, slower runtime)
cargo build

# Build (release - slow compile, fast runtime)
cargo build --release

# Run (debug)
cargo run

# Run (release)
cargo run --release

# Clean build artifacts
cargo clean

# Check for errors without building
cargo check

# Run tests
cargo test

# Format code
cargo fmt

# Lint code
cargo clippy
```

## Getting Help

- 📖 Read the full [README.md](README.md)
- 📝 Check [SUMMARY.md](SUMMARY.md) for technical details
- 🐛 Found a bug? Check the Troubleshooting section
- 💡 Want to contribute? See the Development section in README

## System Requirements

**Minimum:**
- Windows 10 or later
- Rust 1.70+
- 2GB RAM
- DirectX 12 capable GPU
- 100MB disk space

**Recommended:**
- Windows 11
- Rust 1.75+
- 4GB RAM
- Dedicated GPU with latest drivers
- SSD storage

---

**Ready to trade (well, visualize)? Run `cargo run --release` and enjoy! 📈**