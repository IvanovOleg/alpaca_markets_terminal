// Chart module for candlestick charts

use alpaca_markets::Bar;
use chrono::{Datelike, Timelike};

/// Chart state containing all chart-related fields
pub struct Chart {
    pub symbol: String,
    pub symbol_input: String,
    pub timeframe: String,
    pub bars: Vec<Bar>,
    pub loading: bool,
    pub error: Option<String>,
    pub input_focused: bool,
    // Market data stream
    pub market_data_connected: bool,
    pub last_bar_time: Option<String>,
    pub last_bar_symbol: Option<String>,
    pub last_bar_open: Option<String>,
    pub last_bar_high: Option<String>,
    pub last_bar_low: Option<String>,
    pub last_bar_close: Option<String>,
    pub last_bar_volume: Option<String>,
    // Crosshair tracking
    pub mouse_position: Option<gpui::Point<gpui::Pixels>>,
    pub show_crosshair: bool,
    pub chart_bounds: Option<(f32, f32)>, // (width, height) in pixels
    // Bar limit
    pub bar_limit: String,
    pub bar_limit_focused: bool,
    // Chart scroll offset
    pub chart_scroll_offset: f32,
    // Bars per screen (for zoom control)
    pub bars_per_screen: usize,
}

impl Chart {
    pub fn new(symbol: String, timeframe: String) -> Self {
        Self {
            symbol: symbol.clone(),
            symbol_input: symbol,
            timeframe,
            bars: Vec::new(),
            loading: true,
            error: None,
            input_focused: false,
            market_data_connected: false,
            last_bar_time: None,
            last_bar_symbol: None,
            last_bar_open: None,
            last_bar_high: None,
            last_bar_low: None,
            last_bar_close: None,
            last_bar_volume: None,
            mouse_position: None,
            show_crosshair: false,
            chart_bounds: None,
            bar_limit: "100".to_string(),
            bar_limit_focused: false,
            chart_scroll_offset: 0.0,
            bars_per_screen: 100,
        }
    }
}

/// Calculate nice round grid values for price display
pub fn calculate_round_grid_values(min: f64, max: f64, target_count: usize) -> Vec<f64> {
    let range = max - min;
    if range <= 0.0 {
        return vec![min];
    }

    let rough_step = range / target_count as f64;
    let magnitude = 10_f64.powf(rough_step.log10().floor());
    let min_step = range / 20.0;

    let candidates = vec![
        magnitude * 1.0,
        magnitude * 2.0,
        magnitude * 2.5,
        magnitude * 4.0,
        magnitude * 5.0,
        magnitude * 10.0,
        magnitude * 0.5,
    ];

    let candidates: Vec<f64> = candidates
        .into_iter()
        .filter(|&step| step >= min_step)
        .collect();

    let candidates = if candidates.is_empty() {
        vec![min_step]
    } else {
        candidates
    };

    let mut best_values = Vec::new();
    let mut best_diff = usize::MAX;

    for &step in &candidates {
        let start = (min / step).floor() * step;
        let mut values = Vec::new();
        let mut current = start;

        while current <= max + step * 0.001 {
            if current >= min - step * 0.001 {
                values.push(current);
            }
            current += step;
        }

        let diff = if values.len() > target_count {
            values.len() - target_count
        } else {
            target_count - values.len()
        };

        if diff < best_diff {
            best_diff = diff;
            best_values = values;
        }
    }

    best_values
}

/// Align a timestamp to the chart's timeframe boundary
/// This ensures that multiple bar updates within the same timeframe period
/// are recognized as belonging to the same candle
pub fn align_timestamp_to_timeframe(
    timestamp: chrono::DateTime<chrono::Utc>,
    timeframe: &str,
) -> chrono::DateTime<chrono::Utc> {
    match timeframe {
        "1Min" => timestamp
            .with_second(0)
            .unwrap()
            .with_nanosecond(0)
            .unwrap(),
        "5Min" => {
            let minute = timestamp.minute();
            let aligned_minute = (minute / 5) * 5;
            timestamp
                .with_minute(aligned_minute)
                .unwrap()
                .with_second(0)
                .unwrap()
                .with_nanosecond(0)
                .unwrap()
        }
        "15Min" => {
            let minute = timestamp.minute();
            let aligned_minute = (minute / 15) * 15;
            timestamp
                .with_minute(aligned_minute)
                .unwrap()
                .with_second(0)
                .unwrap()
                .with_nanosecond(0)
                .unwrap()
        }
        "1Hour" => timestamp
            .with_minute(0)
            .unwrap()
            .with_second(0)
            .unwrap()
            .with_nanosecond(0)
            .unwrap(),
        "1Day" => timestamp
            .with_hour(0)
            .unwrap()
            .with_minute(0)
            .unwrap()
            .with_second(0)
            .unwrap()
            .with_nanosecond(0)
            .unwrap(),
        "1Week" => {
            let days_from_monday = timestamp.weekday().num_days_from_monday();
            let aligned = timestamp - chrono::Duration::days(days_from_monday as i64);
            aligned
                .with_hour(0)
                .unwrap()
                .with_minute(0)
                .unwrap()
                .with_second(0)
                .unwrap()
                .with_nanosecond(0)
                .unwrap()
        }
        "1Month" => timestamp
            .with_day(1)
            .unwrap()
            .with_hour(0)
            .unwrap()
            .with_minute(0)
            .unwrap()
            .with_second(0)
            .unwrap()
            .with_nanosecond(0)
            .unwrap(),
        _ => timestamp,
    }
}

/// Convert a bar update from the stream to a Bar struct
pub fn convert_bar_update_to_bar(bar_update: &crate::stream::BarUpdate) -> Result<Bar, String> {
    let timestamp = chrono::DateTime::parse_from_rfc3339(&bar_update.timestamp)
        .map_err(|e| format!("Failed to parse timestamp: {}", e))?
        .with_timezone(&chrono::Utc);

    let open = bar_update
        .open
        .parse::<f64>()
        .map_err(|e| format!("Failed to parse open: {}", e))?;
    let high = bar_update
        .high
        .parse::<f64>()
        .map_err(|e| format!("Failed to parse high: {}", e))?;
    let low = bar_update
        .low
        .parse::<f64>()
        .map_err(|e| format!("Failed to parse low: {}", e))?;
    let close = bar_update
        .close
        .parse::<f64>()
        .map_err(|e| format!("Failed to parse close: {}", e))?;
    let volume = bar_update
        .volume
        .parse::<u64>()
        .map_err(|e| format!("Failed to parse volume: {}", e))?;

    Ok(Bar {
        timestamp,
        open,
        high,
        low,
        close,
        volume,
        trade_count: bar_update.trade_count,
        vwap: bar_update.vwap.as_ref().and_then(|v| v.parse::<f64>().ok()),
    })
}
