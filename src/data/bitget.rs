//! Bitget REST 客户端（V2 Spot Market API）
//!
//! 作为 Binance / Bybit 之外的补充数据源。
//!
//! 参考文档：<https://www.bitget.com/api-doc/spot/market/Get-Candle-Data>
//!
//! 关键差异：
//! - URL 格式：`/api/v2/spot/market/candles?symbol=BTCUSDT&granularity=4h&limit=1000`
//! - K 线每条为数组 `[ts, open, high, low, close, base_vol, quote_vol, usdt_vol]`
//!   * 时间字段 `ts` 为毫秒字符串
//!   * 数值字段为字符串
//! - Granularity 代码：`4h → 4h`（与 Bybit 不同，更接近自然语言）
//! - 单次最多 1000 根
//! - 返回**时间升序**（与 Binance 对齐，无需反转）

use serde_json::Value;

use super::kline::{Kline, Timeframe};

/// Bitget 行情客户端（Spot V2）
pub struct Bitget {
    base: String,
    agent: ureq::Agent,
}

impl Bitget {
    pub fn new(base: impl Into<String>) -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout(std::time::Duration::from_secs(30))
            .user_agent("aura-trade/0.1 (+https://github.com/local)")
            .build();
        Self { base: base.into(), agent }
    }

    /// 单次获取 K 线。Bitget V2 spot 单次最多 1000 根。
    pub fn klines(
        &self,
        symbol: &str,
        tf: Timeframe,
        limit: usize,
    ) -> Result<Vec<Kline>, String> {
        let url = format!(
            "{}/api/v2/spot/market/candles?symbol={}&granularity={}&limit={}",
            self.base,
            symbol.to_uppercase(),
            tf_to_bitget(tf),
            limit.min(1000).max(1),
        );
        self.fetch_klines(&url, tf)
    }

    /// 按时间区间批量获取 K 线（自动分页）
    ///
    /// Bitget 的 `startTime` / `endTime` 为毫秒 UTC 时间戳。
    pub fn klines_range(
        &self,
        symbol: &str,
        tf: Timeframe,
        start_ms: i64,
        end_ms: i64,
    ) -> Result<Vec<Kline>, String> {
        let mut out = Vec::new();
        let mut cursor = start_ms;
        loop {
            let url = format!(
                "{}/api/v2/spot/market/candles?symbol={}&granularity={}&startTime={}&endTime={}&limit=1000",
                self.base,
                symbol.to_uppercase(),
                tf_to_bitget(tf),
                cursor,
                end_ms,
            );
            let chunk = self.fetch_klines(&url, tf)?;
            if chunk.is_empty() {
                break;
            }
            let last_close = chunk.last().map(|k| k.close_time).unwrap_or(cursor);
            out.extend(chunk);
            if last_close >= end_ms || out.len() >= 1_000_000 {
                break;
            }
            cursor = last_close + 1;
        }
        Ok(out)
    }

    /// 拉取 Bitget Spot 所有 online 状态的 USDT 交易对
    ///
    /// 接口：`GET /api/v2/spot/public/symbols`
    /// 返回 `data[].symbol / status / baseCoin / quoteCoin`
    pub fn symbols_spot_usdt(&self) -> Result<Vec<String>, String> {
        let url = format!("{}/api/v2/spot/public/symbols", self.base);
        log::debug!("Bitget GET {}", url);
        let resp = self
            .agent
            .get(&url)
            .call()
            .map_err(|e| format!("Bitget symbols 请求失败: {}", e))?;
        let v: Value = resp
            .into_json()
            .map_err(|e| format!("Bitget symbols 非 JSON: {}", e))?;
        // 检查 code
        if let Some(code) = v.get("code").and_then(|x| x.as_str()) {
            if code != "00000" {
                let msg = v.get("msg").and_then(|x| x.as_str()).unwrap_or("unknown");
                return Err(format!("Bitget code={} msg={}", code, msg));
            }
        }
        let arr = v
            .get("data")
            .and_then(|d| d.as_array())
            .ok_or("Bitget symbols 缺少 data 数组")?;
        let mut out: Vec<String> = Vec::with_capacity(arr.len());
        for item in arr {
            let status = item.get("status").and_then(|x| x.as_str()).unwrap_or("");
            let sym = item.get("symbol").and_then(|x| x.as_str()).unwrap_or("");
            let quote = item.get("quoteCoin").and_then(|x| x.as_str()).unwrap_or("");
            if status == "online" && quote == "USDT" && !sym.is_empty() {
                out.push(sym.to_string());
            }
        }
        out.sort();
        out.dedup();
        Ok(out)
    }

    fn fetch_klines(&self, url: &str, tf: Timeframe) -> Result<Vec<Kline>, String> {
        log::debug!("Bitget GET {}", url);
        let resp = self
            .agent
            .get(url)
            .call()
            .map_err(|e| format!("Bitget 请求失败: {}", e))?;
        let v: Value = resp
            .into_json()
            .map_err(|e| format!("Bitget 返回非 JSON: {}", e))?;
        parse_kline_result(&v, tf)
    }
}

/// 将 Timeframe 映射到 Bitget 的 granularity 代码
fn tf_to_bitget(tf: Timeframe) -> &'static str {
    match tf {
        Timeframe::M1 => "1min",
        Timeframe::M5 => "5min",
        Timeframe::M15 => "15min",
        Timeframe::M30 => "30min",
        Timeframe::H1 => "1h",
        Timeframe::H4 => "4h",
        Timeframe::D1 => "1day",
        Timeframe::W1 => "1week",
        Timeframe::Mo1 => "1M",
    }
}

/// 解析 Bitget 的 kline 响应
///
/// 返回结构：
/// ```json
/// {
///   "code": "00000",
///   "msg": "success",
///   "data": [
///     ["1776384000000", "74150.5", "74832.5", "73900.0", "74583.2", "1023.5", "76123456.7", "76123456.7"],
///     ...
///   ]
/// }
/// ```
/// 注意：Bitget 返回时间升序（与 Binance 对齐）。
fn parse_kline_result(v: &Value, tf: Timeframe) -> Result<Vec<Kline>, String> {
    // 检查业务 code
    if let Some(code) = v.get("code").and_then(|x| x.as_str()) {
        if code != "00000" {
            let msg = v.get("msg").and_then(|x| x.as_str()).unwrap_or("unknown");
            return Err(format!("Bitget code={} msg={}", code, msg));
        }
    }
    let arr = v
        .get("data")
        .and_then(|d| d.as_array())
        .ok_or("Bitget 响应缺少 data 数组")?;
    let interval_ms = tf.interval_ms();
    let mut out: Vec<Kline> = Vec::with_capacity(arr.len());
    for (i, row) in arr.iter().enumerate() {
        let row = row
            .as_array()
            .ok_or_else(|| format!("第 {} 行不是数组", i))?;
        if row.len() < 6 {
            return Err(format!("第 {} 行字段数 {} < 6", i, row.len()));
        }
        let start = parse_num(&row[0], "ts")? as i64;
        let open = parse_num(&row[1], "open")?;
        let high = parse_num(&row[2], "high")?;
        let low = parse_num(&row[3], "low")?;
        let close = parse_num(&row[4], "close")?;
        let volume = parse_num(&row[5], "volume")?;
        let close_time = start + interval_ms - 1;
        out.push(Kline {
            open_time: start,
            open,
            high,
            low,
            close,
            volume,
            close_time,
        });
    }
    // Bitget 已经是升序，但保险起见再排一次
    out.sort_by_key(|k| k.open_time);
    Ok(out)
}

fn parse_num(v: &Value, field: &str) -> Result<f64, String> {
    if let Some(s) = v.as_str() {
        s.parse().map_err(|e| format!("{} 解析失败: {}", field, e))
    } else if let Some(f) = v.as_f64() {
        Ok(f)
    } else if let Some(i) = v.as_i64() {
        Ok(i as f64)
    } else {
        Err(format!("{} 既非字符串也非数字", field))
    }
}
