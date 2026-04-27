//! Binance REST 客户端
//!
//! 只使用 ureq（纯同步）+ 公开行情 API，不涉及任何交易 API。
//!
//! 参考文档：<https://binance-docs.github.io/apidocs/spot/en/#kline-candlestick-data>

use serde_json::Value;

use super::kline::{Kline, Timeframe};

/// Binance 行情客户端
pub struct Binance {
    base: String,
    agent: ureq::Agent,
}

impl Binance {
    pub fn new(base: impl Into<String>) -> Self {
        // 30 秒超时，避免个别请求阻塞整个服务
        let agent = ureq::AgentBuilder::new()
            .timeout(std::time::Duration::from_secs(30))
            .user_agent("aura-trade/0.1 (+https://github.com/local)")
            .build();
        Self { base: base.into(), agent }
    }

    /// 获取 K线。Binance 单次最多 1000 根。
    pub fn klines(
        &self,
        symbol: &str,
        tf: Timeframe,
        limit: usize,
    ) -> Result<Vec<Kline>, String> {
        let url = format!(
            "{}/api/v3/klines?symbol={}&interval={}&limit={}",
            self.base,
            symbol.to_uppercase(),
            tf.as_str(),
            limit.min(1000).max(1)
        );
        self.fetch_klines(&url)
    }

    /// 按时间区间批量获取 K线（自动分页，以 1000 根为单位）
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
                "{}/api/v3/klines?symbol={}&interval={}&startTime={}&endTime={}&limit=1000",
                self.base,
                symbol.to_uppercase(),
                tf.as_str(),
                cursor,
                end_ms
            );
            let chunk = self.fetch_klines(&url)?;
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

    /// 拉取所有 TRADING 状态的 USDT 永续现货交易对名（仅 spot）。
    /// 只取 `symbols[].symbol`，按字典序排序。
    pub fn exchange_info_usdt_symbols(&self) -> Result<Vec<String>, String> {
        let url = format!("{}/api/v3/exchangeInfo", self.base);
        log::debug!("Binance GET {}", url);
        let resp = self
            .agent
            .get(&url)
            .call()
            .map_err(|e| format!("Binance exchangeInfo 失败: {}", e))?;
        let value: Value = resp
            .into_json()
            .map_err(|e| format!("Binance exchangeInfo 返回非 JSON: {}", e))?;
        let arr = value
            .get("symbols")
            .and_then(|v| v.as_array())
            .ok_or("exchangeInfo 缺少 symbols 数组")?;
        let mut out: Vec<String> = Vec::with_capacity(arr.len());
        for item in arr {
            let status = item.get("status").and_then(|v| v.as_str()).unwrap_or("");
            let quote = item.get("quoteAsset").and_then(|v| v.as_str()).unwrap_or("");
            let sym = item.get("symbol").and_then(|v| v.as_str()).unwrap_or("");
            if status == "TRADING" && quote == "USDT" && !sym.is_empty() {
                out.push(sym.to_string());
            }
        }
        out.sort();
        out.dedup();
        Ok(out)
    }

    fn fetch_klines(&self, url: &str) -> Result<Vec<Kline>, String> {
        log::debug!("Binance GET {}", url);
        let resp = self
            .agent
            .get(url)
            .call()
            .map_err(|e| format!("Binance 请求失败: {}", e))?;
        let value: Value = resp
            .into_json()
            .map_err(|e| format!("Binance 返回非 JSON: {}", e))?;
        parse_kline_array(&value)
    }
}

/// 解析 Binance 的 K线返回：二维数组
fn parse_kline_array(v: &Value) -> Result<Vec<Kline>, String> {
    let arr = v.as_array().ok_or("期望顶层为数组")?;
    let mut out = Vec::with_capacity(arr.len());
    for (i, row) in arr.iter().enumerate() {
        let row = row
            .as_array()
            .ok_or_else(|| format!("第 {} 行不是数组", i))?;
        if row.len() < 7 {
            return Err(format!("第 {} 行字段数 {} < 7", i, row.len()));
        }
        let open_time = row[0].as_i64().ok_or("open_time 非整数")?;
        let open = parse_f64(&row[1], "open")?;
        let high = parse_f64(&row[2], "high")?;
        let low = parse_f64(&row[3], "low")?;
        let close = parse_f64(&row[4], "close")?;
        let volume = parse_f64(&row[5], "volume")?;
        let close_time = row[6].as_i64().ok_or("close_time 非整数")?;
        out.push(Kline { open_time, open, high, low, close, volume, close_time });
    }
    Ok(out)
}

fn parse_f64(v: &Value, field: &str) -> Result<f64, String> {
    if let Some(s) = v.as_str() {
        s.parse().map_err(|e| format!("{} 解析失败: {}", field, e))
    } else if let Some(f) = v.as_f64() {
        Ok(f)
    } else {
        Err(format!("{} 既非字符串也非数字", field))
    }
}
