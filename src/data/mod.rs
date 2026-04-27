//! 数据层：行情采集与本地缓存
//!
//! - [`kline`]    K线结构体 + 时间框架枚举
//! - [`binance`]  Binance 公开 REST 客户端
//! - [`bybit`]    Bybit V5 公开 REST 客户端
//! - [`bitget`]   Bitget V2 公开 REST 客户端
//! - [`okx`]      OKX V5 公开 REST 客户端
//! - [`cache`]    本地文件缓存（多交易所路由：按 symbol 前缀分发）

pub mod binance;
pub mod bitget;
pub mod bybit;
pub mod cache;
pub mod kline;
pub mod okx;

pub use binance::Binance;
pub use bitget::Bitget;
pub use bybit::Bybit;
pub use cache::{Exchange, KlineCache};
pub use kline::{Kline, Timeframe};
pub use okx::Okx;
