//! Bybit REST 客户端（V5 Spot Market API）
//!
//! 作为 Binance 的备选 / 并列数据源。当某个 IP 被 Binance 封禁（418）时，
//! 仍可通过 Bybit 获取同一 symbol 的 K 线。
//!
//! 参考文档：<https://bybit-exchange.github.io/docs/v5/market/kline>
//!
//! 与 Binance 的差异：
//! - URL 格式：`/v5/market/kline?category=spot&symbol=BTCUSDT&interval=240&limit=500`
//! - 返回 **倒序**（最新在前），需反转成时间升序
//! - K 线每条为 `[start, open, high, low, close, volume, turnover]` 共 7 个**字符串**
//!   * Binance 的 `close_time` 字段在 Bybit 中需要计算：`close_time = start + interval_ms - 1`
//! - Interval 代码不同：`4h → 240`，`1d → D`，`1w → W`，`1M → M`

use serde_json::Value;

use super::kline::{Kline, Timeframe};

/// Bybit 行情客户端（Spot V5）
pub struct Bybit {
    base: String,
    agent: ureq::Agent,
}

impl Bybit {
    pub fn new(base: impl Into<String>) -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout(std::time::Duration::from_secs(30))
            .user_agent("aura-trade/0.1 (+https://github.com/local)")
            .build();
        Self { base: base.into(), agent }
    }

    /// 单次获取 K 线。Bybit V5 spot 单次最多 1000 根。
    pub fn klines(
        &self,
        symbol: &str,
        tf: Timeframe,
        limit: usize,
    ) -> Result<Vec<Kline>, String> {
        let url = format!(
            "{}/v5/market/kline?category=spot&symbol={}&interval={}&limit={}",
            self.base,
            symbol.to_uppercase(),
            tf_to_bybit(tf),
            limit.min(1000).max(1),
        );
        self.fetch_klines(&url, tf)
    }

    /// 按时间区间批量获取 K 线（自动分页，以 1000 根为单位）
    ///
    /// Bybit 的 start / end 参数为毫秒 UTC 时间戳。
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
                "{}/v5/market/kline?category=spot&symbol={}&interval={}&start={}&end={}&limit=1000",
                self.base,
                symbol.to_uppercase(),
                tf_to_bybit(tf),
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

    /// 拉取 Bybit Spot 所有 TRADING 状态的 USDT 交易对
    ///
    /// Bybit 的交易对接口：`GET /v5/market/instruments-info?category=spot`
    /// 返回 `result.list[].symbol`（字符串）和 `result.list[].status`（Trading/...）
    pub fn instruments_info_usdt_symbols(&self) -> Result<Vec<String>, String> {
        let url = format!("{}/v5/market/instruments-info?category=spot", self.base);
        log::debug!("Bybit GET {}", url);
        let resp = self
            .agent
            .get(&url)
            .call()
            .map_err(|e| format!("Bybit instruments-info 失败: {}", e))?;
        let v: Value = resp
            .into_json()
            .map_err(|e| format!("Bybit instruments-info 非 JSON: {}", e))?;
        let arr = v
            .get("result")
            .and_then(|r| r.get("list"))
            .and_then(|l| l.as_array())
            .ok_or("instruments-info 缺少 result.list 数组")?;
        let mut out: Vec<String> = Vec::with_capacity(arr.len());
        for item in arr {
            let status = item.get("status").and_then(|x| x.as_str()).unwrap_or("");
            let sym = item.get("symbol").and_then(|x| x.as_str()).unwrap_or("");
            // Bybit spot 的 symbol 已是拼接格式（如 BTCUSDT），挑 USDT 报价即可
            if status == "Trading" && sym.ends_with("USDT") && !sym.is_empty() {
                out.push(sym.to_string());
            }
        }
        out.sort();
        out.dedup();
        Ok(out)
    }

    fn fetch_klines(&self, url: &str, tf: Timeframe) -> Result<Vec<Kline>, String> {
        log::debug!("Bybit GET {}", url);
        let resp = self
            .agent
            .get(url)
            .call()
            .map_err(|e| format!("Bybit 请求失败: {}", e))?;
        let v: Value = resp
            .into_json()
            .map_err(|e| format!("Bybit 返回非 JSON: {}", e))?;
        parse_kline_result(&v, tf)
    }
}

/// 将 Timeframe 映射到 Bybit 的 interval 代码
fn tf_to_bybit(tf: Timeframe) -> &'static str {
    match tf {
        Timeframe::M1 => "1",
        Timeframe::M5 => "5",
        Timeframe::M15 => "15",
        Timeframe::M30 => "30",
        Timeframe::H1 => "60",
        Timeframe::H4 => "240",
        Timeframe::D1 => "D",
        Timeframe::W1 => "W",
        Timeframe::Mo1 => "M",
    }
}

/// 解析 Bybit 的 kline 响应
///
/// 返回结构示例：
/// ```json
/// {
///   "retCode": 0,
///   "result": {
///     "symbol": "BTCUSDT",
///     "category": "spot",
///     "list": [
///       ["1776672000000", "74832.5", "75403.2", "74615.2", "75243.8", "1505.86", "1129.2M"],
///       ...
///     ]
///   }
/// }
/// ```
/// **注意**：`list` 按 start 时间 **倒序**（最新在前），需反转。
fn parse_kline_result(v: &Value, tf: Timeframe) -> Result<Vec<Kline>, String> {
    // 检查业务 retCode
    if let Some(code) = v.get("retCode").and_then(|x| x.as_i64()) {
        if code != 0 {
            let msg = v
                .get("retMsg")
                .and_then(|x| x.as_str())
                .unwrap_or("unknown");
            return Err(format!("Bybit retCode={} msg={}", code, msg));
        }
    }
    let arr = v
        .get("result")
        .and_then(|r| r.get("list"))
        .and_then(|l| l.as_array())
        .ok_or("Bybit 响应缺少 result.list")?;
    let interval_ms = tf.interval_ms();
    let mut out: Vec<Kline> = Vec::with_capacity(arr.len());
    for (i, row) in arr.iter().enumerate() {
        let row = row
            .as_array()
            .ok_or_else(|| format!("第 {} 行不是数组", i))?;
        if row.len() < 6 {
            return Err(format!("第 {} 行字段数 {} < 6", i, row.len()));
        }
        let start = parse_num(&row[0], "start")? as i64;
        let open = parse_num(&row[1], "open")?;
        let high = parse_num(&row[2], "high")?;
        let low = parse_num(&row[3], "low")?;
        let close = parse_num(&row[4], "close")?;
        let volume = parse_num(&row[5], "volume")?;
        // Bybit 不返回 close_time → 按 interval 推算
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
    // 反转为时间升序（与 Binance 对齐）
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
