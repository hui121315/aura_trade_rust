//! OKX REST 客户端（V5 Market API）
//!
//! 作为 Binance / Bybit / Bitget 之外的补充数据源。
//!
//! 参考文档：<https://www.okx.com/docs-v5/en/#market-data-rest-api>
//!
//! 关键差异：
//! - URL 格式：`/api/v5/market/candles?instId=BTC-USDT&bar=4H&limit=300`
//! - OKX 的 symbol 使用连字符（`BTC-USDT`），而内部统一格式无连字符（`BTCUSDT`）
//!   * 对外暴露的 symbol 是 `BTCUSDT`，内部请求前加连字符
//! - K 线每条为 `[ts, open, high, low, close, vol, volCcy, volCcyQuote, confirm]`
//! - 单次最多 300 根（远少于其他交易所）
//! - 返回**时间倒序**（最新在前），需反转
//! - Granularity 代码：`4H`（大写 H），`1m` / `1D` / `1W` 等

use serde_json::Value;

use super::kline::{Kline, Timeframe};

/// OKX 行情客户端（V5 Market）
pub struct Okx {
    base: String,
    agent: ureq::Agent,
}

impl Okx {
    pub fn new(base: impl Into<String>) -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout(std::time::Duration::from_secs(30))
            .user_agent("aura-trade/0.1 (+https://github.com/local)")
            .build();
        Self { base: base.into(), agent }
    }

    /// 单次获取 K 线。OKX V5 单次最多 300 根。
    pub fn klines(
        &self,
        symbol: &str,
        tf: Timeframe,
        limit: usize,
    ) -> Result<Vec<Kline>, String> {
        let inst_id = to_okx_inst_id(symbol);
        let url = format!(
            "{}/api/v5/market/candles?instId={}&bar={}&limit={}",
            self.base,
            inst_id,
            tf_to_okx(tf),
            limit.min(300).max(1),
        );
        self.fetch_klines(&url, tf)
    }

    /// 按时间区间批量获取 K 线（自动分页）
    ///
    /// OKX 使用 `after`（拉取该时间**之前**的数据，即向过去拉）做游标分页。
    /// 返回时间倒序（最新在前），我们循环调用直到 chunk 首根早于 start_ms。
    pub fn klines_range(
        &self,
        symbol: &str,
        tf: Timeframe,
        start_ms: i64,
        end_ms: i64,
    ) -> Result<Vec<Kline>, String> {
        let inst_id = to_okx_inst_id(symbol);
        let mut out: Vec<Kline> = Vec::new();
        let mut cursor = end_ms;
        // 最多拉 1000 次（300 × 1000 = 30 万根）防御性上限
        for _ in 0..1000 {
            let url = format!(
                "{}/api/v5/market/history-candles?instId={}&bar={}&after={}&limit=300",
                self.base,
                inst_id,
                tf_to_okx(tf),
                cursor,
            );
            let chunk = self.fetch_klines(&url, tf)?;
            if chunk.is_empty() {
                break;
            }
            // chunk 本身是升序（parse_kline_result 已反转排序）
            let first_ts = chunk.first().map(|k| k.open_time).unwrap_or(cursor);
            let last_ts = chunk.last().map(|k| k.open_time).unwrap_or(cursor);
            // 追加
            out.extend(chunk);
            // 若已经覆盖到 start_ms 或更早，停止
            if first_ts <= start_ms {
                break;
            }
            // OKX 的 `after` 表示拉取该时间之前的数据，所以 cursor 用当前批次最早时间
            let next = first_ts - 1;
            if next >= cursor {
                break;
            }
            cursor = next;
            if last_ts < start_ms {
                break;
            }
        }
        // 去重（可能分页交叠）
        out.sort_by_key(|k| k.open_time);
        out.dedup_by_key(|k| k.open_time);
        // 过滤到 [start_ms, end_ms]
        out.retain(|k| k.open_time >= start_ms && k.open_time <= end_ms);
        Ok(out)
    }

    /// 拉取 OKX Spot 所有 live 状态的 USDT 交易对
    ///
    /// 接口：`GET /api/v5/public/instruments?instType=SPOT`
    /// 返回 `data[].instId / state / baseCcy / quoteCcy`
    pub fn instruments_spot_usdt(&self) -> Result<Vec<String>, String> {
        let url = format!("{}/api/v5/public/instruments?instType=SPOT", self.base);
        log::debug!("OKX GET {}", url);
        let resp = self
            .agent
            .get(&url)
            .call()
            .map_err(|e| format!("OKX instruments 请求失败: {}", e))?;
        let v: Value = resp
            .into_json()
            .map_err(|e| format!("OKX instruments 非 JSON: {}", e))?;
        if let Some(code) = v.get("code").and_then(|x| x.as_str()) {
            if code != "0" {
                let msg = v.get("msg").and_then(|x| x.as_str()).unwrap_or("unknown");
                return Err(format!("OKX code={} msg={}", code, msg));
            }
        }
        let arr = v
            .get("data")
            .and_then(|d| d.as_array())
            .ok_or("OKX instruments 缺少 data 数组")?;
        let mut out: Vec<String> = Vec::with_capacity(arr.len());
        for item in arr {
            let state = item.get("state").and_then(|x| x.as_str()).unwrap_or("");
            let inst_id = item.get("instId").and_then(|x| x.as_str()).unwrap_or("");
            let quote = item.get("quoteCcy").and_then(|x| x.as_str()).unwrap_or("");
            if state == "live" && quote == "USDT" && !inst_id.is_empty() {
                // 内部存储去掉连字符 → "BTCUSDT"
                let sym = inst_id.replace('-', "");
                out.push(sym);
            }
        }
        out.sort();
        out.dedup();
        Ok(out)
    }

    fn fetch_klines(&self, url: &str, tf: Timeframe) -> Result<Vec<Kline>, String> {
        log::debug!("OKX GET {}", url);
        let resp = self
            .agent
            .get(url)
            .call()
            .map_err(|e| format!("OKX 请求失败: {}", e))?;
        let v: Value = resp
            .into_json()
            .map_err(|e| format!("OKX 返回非 JSON: {}", e))?;
        parse_kline_result(&v, tf)
    }
}

/// 将内部 symbol (BTCUSDT) 转换为 OKX 的 instId (BTC-USDT)
fn to_okx_inst_id(symbol: &str) -> String {
    let up = symbol.to_uppercase();
    if up.ends_with("USDT") && !up.contains('-') {
        let base = &up[..up.len() - 4];
        format!("{}-USDT", base)
    } else if up.contains('-') {
        up
    } else {
        up
    }
}

/// 将 Timeframe 映射到 OKX 的 bar 代码
fn tf_to_okx(tf: Timeframe) -> &'static str {
    match tf {
        Timeframe::M1 => "1m",
        Timeframe::M5 => "5m",
        Timeframe::M15 => "15m",
        Timeframe::M30 => "30m",
        Timeframe::H1 => "1H",
        Timeframe::H4 => "4H",
        Timeframe::D1 => "1D",
        Timeframe::W1 => "1W",
        Timeframe::Mo1 => "1M",
    }
}

/// 解析 OKX 的 kline 响应
///
/// 返回结构：
/// ```json
/// {
///   "code": "0",
///   "msg": "",
///   "data": [
///     ["1776672000000", "74832.5", "75403.2", "74615.2", "75243.8", "1505.86", "...", "...", "1"],
///     ...
///   ]
/// }
/// ```
/// **注意**：`data` 按时间 **倒序**（最新在前），需反转。
fn parse_kline_result(v: &Value, tf: Timeframe) -> Result<Vec<Kline>, String> {
    if let Some(code) = v.get("code").and_then(|x| x.as_str()) {
        if code != "0" {
            let msg = v.get("msg").and_then(|x| x.as_str()).unwrap_or("unknown");
            return Err(format!("OKX code={} msg={}", code, msg));
        }
    }
    let arr = v
        .get("data")
        .and_then(|d| d.as_array())
        .ok_or("OKX 响应缺少 data 数组")?;
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
    // 反转为升序
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
