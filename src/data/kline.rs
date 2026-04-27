//! K线数据结构
//!
//! 对齐 PRD 中所有计算引擎的输入格式。时间戳使用毫秒 UTC。

use serde::{Deserialize, Serialize};

/// 单根 K线（OHLCV + 时间）
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Kline {
    /// 开盘时间 (毫秒, UTC)
    pub open_time: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    /// 收盘时间 (毫秒, UTC)
    pub close_time: i64,
}

impl Kline {
    /// K线是否为阳线
    #[inline]
    pub fn is_bullish(&self) -> bool {
        self.close > self.open
    }

    /// 实体长度 |close - open|
    #[inline]
    pub fn body(&self) -> f64 {
        (self.close - self.open).abs()
    }

    /// 全天波幅 high - low
    #[inline]
    pub fn range(&self) -> f64 {
        self.high - self.low
    }

    /// 上影线
    #[inline]
    pub fn upper_shadow(&self) -> f64 {
        self.high - self.open.max(self.close)
    }

    /// 下影线
    #[inline]
    pub fn lower_shadow(&self) -> f64 {
        self.open.min(self.close) - self.low
    }
}

/// 时间框架枚举（与 Binance interval 一一对应）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Timeframe {
    M1,
    M5,
    M15,
    M30,
    H1,
    H4,
    D1,
    W1,
    Mo1,
}

impl Timeframe {
    pub fn as_str(&self) -> &'static str {
        match self {
            Timeframe::M1 => "1m",
            Timeframe::M5 => "5m",
            Timeframe::M15 => "15m",
            Timeframe::M30 => "30m",
            Timeframe::H1 => "1h",
            Timeframe::H4 => "4h",
            Timeframe::D1 => "1d",
            Timeframe::W1 => "1w",
            Timeframe::Mo1 => "1M",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "1m" => Timeframe::M1,
            "5m" => Timeframe::M5,
            "15m" => Timeframe::M15,
            "30m" => Timeframe::M30,
            "1h" => Timeframe::H1,
            "4h" => Timeframe::H4,
            "1d" => Timeframe::D1,
            "1w" => Timeframe::W1,
            "1M" => Timeframe::Mo1,
            _ => return None,
        })
    }

    /// 该周期对应的毫秒数（Mo1 取 30 天估算，仅用于容量估算）
    pub fn interval_ms(&self) -> i64 {
        match self {
            Timeframe::M1 => 60_000,
            Timeframe::M5 => 5 * 60_000,
            Timeframe::M15 => 15 * 60_000,
            Timeframe::M30 => 30 * 60_000,
            Timeframe::H1 => 60 * 60_000,
            Timeframe::H4 => 4 * 60 * 60_000,
            Timeframe::D1 => 24 * 60 * 60_000,
            Timeframe::W1 => 7 * 24 * 60 * 60_000,
            Timeframe::Mo1 => 30 * 24 * 60 * 60_000,
        }
    }
}
