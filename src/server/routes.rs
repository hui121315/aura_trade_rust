//! 路由分发器
//!
//! 后续 Phase 的每一个 API 端点都在这里注册。
//! 匹配规则使用最朴素的 `(method, path_prefix)` 分支。

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;

use tiny_http::{Method, Request, Response};

use crate::config::Config;
use crate::data::{Exchange, KlineCache, Timeframe};
use crate::engine::backtest::{self, BacktestConfig, Playbook, StopKind};
use crate::engine::candle;
use crate::engine::chartpattern;
use crate::engine::effectiveness;
use crate::engine::indicator;
use crate::engine::ma::{self, MaKind};
use crate::engine::resonance;
use crate::engine::rl;
use crate::engine::signal;
use crate::engine::trend;

use super::response::{json_err, json_ok};
use super::static_files;

/// Sprint B 自动化 3/3：实时增量学习的 debounce 间隔（毫秒）
///
/// 每次调用 `/api/decision` / `/api/signals` / `/api/resonance` 时，若距离
/// 上次对同一 (symbol, interval) 做过 merge 已经 ≥ 此间隔，则触发后台线程
/// 重新 evaluate + merge_report。对单次请求 0 开销，背后后台线程每 5 分钟
/// 对每个被访问的组合"充电"一次。
pub const LIVE_LEARN_DEBOUNCE_MS: i64 = 5 * 60 * 1000;

/// 请求上下文 / 应用状态
pub struct Ctx {
    pub cfg: Arc<Config>,
    pub cache: Arc<KlineCache>,
    /// Sprint B：Bandit 持久化状态（全进程共享）
    pub bandit: Arc<Mutex<rl::BanditState>>,
    /// Sprint B 3/3：每个 (symbol, interval) 上次 live-learn 的时间戳（ms）
    pub live_learn_last: Arc<Mutex<HashMap<String, i64>>>,
    /// M17：启动时后台跑的 hardcoded seed benchmark 快照（内存缓存，不持久化）
    /// key = seed_id, value = `Vec<BenchmarkSnapshot>`
    pub seed_benchmarks:
        Arc<Mutex<HashMap<String, Vec<crate::engine::system::BenchmarkSnapshot>>>>,
    /// 多交易所现货 USDT 交易对聚合缓存（30 分钟 TTL）
    /// 每条记录为 `(exchange, native_symbol)`，由前端拼成 `EXCHANGE:SYMBOL` 作为统一 ID
    pub symbols_cache: Arc<Mutex<Option<(std::time::Instant, Vec<SymbolEntry>)>>>,
}

/// 聚合 symbol 条目：供前端下拉显示"交易所 + 币种"
#[derive(Debug, Clone, serde::Serialize)]
pub struct SymbolEntry {
    /// 统一 ID，如 `BINANCE:BTCUSDT` 或 `BYBIT:BTCUSDT`（前端保存到配置）
    pub id: String,
    /// 交易所名，大写：`BINANCE` / `BYBIT`
    pub exchange: String,
    /// 原生 symbol，如 `BTCUSDT`
    pub symbol: String,
    /// 基础币（从 symbol 推断）：`BTC`
    pub base: String,
    /// 报价币：`USDT`（当前均为 USDT）
    pub quote: String,
}

/// 路由总入口
pub fn dispatch(ctx: &Ctx, req: Request) {
    let method = req.method().clone();
    let url = req.url().to_string();
    let (path, query) = split_query(&url);

    log::info!("{} {}", method_str(&method), url);

    let result = match (&method, path.as_str()) {
        // --- 系统端点 ---
        (&Method::Get, "/api/ping") => req.respond(handle_ping()),
        (&Method::Get, "/api/version") => req.respond(handle_version()),

        // --- 业务端点（Phase 1.2+） ---
        (&Method::Get, "/api/symbols") => req.respond(handle_symbols(ctx)),
        (&Method::Get, "/api/klines") => req.respond(handle_klines(ctx, &query)),
        (&Method::Get, "/api/ma_state") => req.respond(handle_ma_state(ctx, &query)),
        (&Method::Get, "/api/candle_patterns") => {
            req.respond(handle_candle_patterns(ctx, &query))
        }
        (&Method::Get, "/api/trend_state") => req.respond(handle_trend_state(ctx, &query)),
        (&Method::Get, "/api/chart_patterns") => req.respond(handle_chart_patterns(ctx, &query)),
        (&Method::Get, "/api/resonance") => req.respond(handle_resonance(ctx, &query)),
        (&Method::Get, "/api/signals") => req.respond(handle_signals(ctx, &query)),
        (&Method::Get, "/api/decision") => req.respond(handle_decision(ctx, &query)),
        (&Method::Get, "/api/indicators/series") => {
            req.respond(handle_indicators_series(ctx, &query))
        }
        (&Method::Get, "/api/effectiveness") => req.respond(handle_effectiveness(ctx, &query)),
        (&Method::Get, "/api/bandit/state") => req.respond(handle_bandit_state(ctx)),
        (&Method::Get, "/api/bandit/train") | (&Method::Post, "/api/bandit/train") => {
            req.respond(handle_bandit_train(ctx, &query))
        }
        (&Method::Post, "/api/bandit/reset") => req.respond(handle_bandit_reset(ctx)),
        (&Method::Get, "/api/bandit/decide") => req.respond(handle_bandit_decide(ctx, &query)),
        (&Method::Get, "/api/backtest/run") | (&Method::Post, "/api/backtest/run") => {
            req.respond(handle_backtest(ctx, &query))
        }
        (&Method::Get, "/api/backtest/playbook")
        | (&Method::Post, "/api/backtest/playbook") => {
            req.respond(handle_playbook_backtest(ctx, &query))
        }

        // --- 体系实验室（M4）---
        (&Method::Get, "/api/system/components") => {
            req.respond(super::system_routes::handle_list_components())
        }
        (&Method::Get, "/api/system/seeds") => {
            req.respond(super::system_routes::handle_list_seeds(ctx))
        }
        (&Method::Post, "/api/system/promote") => {
            super::system_routes::handle_promote(ctx, req)
        }
        (&Method::Post, "/api/system/demote") => {
            super::system_routes::handle_demote(ctx, req)
        }
        (&Method::Post, "/api/system/run") => {
            // POST 需要消费 req 而非仅 response，单独处理
            super::system_routes::handle_run(ctx, req)
        }
        (&Method::Post, "/api/system/walkforward") => {
            super::system_routes::handle_walkforward(ctx, req)
        }
        (&Method::Post, "/api/system/discover") => {
            super::system_routes::handle_discover(ctx, req)
        }
        (&Method::Post, "/api/system/benchmark") => {
            super::system_routes::handle_benchmark(ctx, req)
        }
        (&Method::Post, "/api/system/live_scan") => {
            super::system_routes::handle_live_scan(ctx, req)
        }

        // --- 静态前端 ---
        (&Method::Get, _) => static_files::serve(req, &ctx.cfg.web_root, &path),

        // --- 其它方法 ---
        _ => req.respond(json_err(405, "Method Not Allowed")),
    };

    if let Err(e) = result {
        log::warn!("响应失败: {}", e);
    }
}

fn method_str(m: &Method) -> &'static str {
    match m {
        Method::Get => "GET",
        Method::Post => "POST",
        Method::Put => "PUT",
        Method::Delete => "DELETE",
        Method::Patch => "PATCH",
        Method::Head => "HEAD",
        Method::Options => "OPTIONS",
        _ => "OTHER",
    }
}

/// 拆分 `/path?k=v&...` 为 (path, query_map)
fn split_query(url: &str) -> (String, HashMap<String, String>) {
    match url.split_once('?') {
        Some((p, q)) => (p.to_string(), parse_query(q)),
        None => (url.to_string(), HashMap::new()),
    }
}

fn parse_query(q: &str) -> HashMap<String, String> {
    use super::url_decode::decode;
    let mut map = HashMap::new();
    for pair in q.split('&').filter(|s| !s.is_empty()) {
        if let Some((k, v)) = pair.split_once('=') {
            map.insert(decode(k), decode(v));
        } else {
            map.insert(decode(pair), String::new());
        }
    }
    map
}

// --- 系统端点实现 ---

#[derive(serde::Serialize)]
struct Pong {
    pong: bool,
    server: &'static str,
}

fn handle_ping() -> Response<std::io::Cursor<Vec<u8>>> {
    json_ok(Pong { pong: true, server: "aura_trade" })
}

#[derive(serde::Serialize)]
struct VersionInfo {
    name: &'static str,
    version: &'static str,
    prd_version: &'static str,
    phase: &'static str,
}

fn handle_version() -> Response<std::io::Cursor<Vec<u8>>> {
    json_ok(VersionInfo {
        name: env!("CARGO_PKG_NAME"),
        version: env!("CARGO_PKG_VERSION"),
        prd_version: "v3.1",
        phase: "Phase 1 MVP - 均线 + K线形态",
    })
}

// =========================================================
// 业务端点（Phase 1.2 / 1.3 / 1.4）
// =========================================================

// --- /api/symbols -----------------------------------------
// 返回多交易所现货 USDT 交易对聚合列表（30 分钟内存缓存）
//
// 响应结构：
//   {
//     "count": 1234,
//     "exchanges": ["BINANCE", "BYBIT"],
//     "entries": [
//       { "id": "BINANCE:BTCUSDT", "exchange": "BINANCE", "symbol": "BTCUSDT", "base": "BTC", "quote": "USDT" },
//       ...
//     ],
//     "symbols": ["BTCUSDT", "ETHUSDT", ...]   // 向后兼容：Binance 的原 symbols 列表
//   }
#[derive(serde::Serialize)]
struct SymbolsResp {
    count: usize,
    exchanges: Vec<String>,
    entries: Vec<SymbolEntry>,
    /// 向后兼容：Binance 的 symbols 列表（旧前端用）
    symbols: Vec<String>,
}

fn handle_symbols(ctx: &Ctx) -> Response<std::io::Cursor<Vec<u8>>> {
    const TTL_SECS: u64 = 30 * 60;
    // 1) 查缓存
    if let Ok(guard) = ctx.symbols_cache.lock() {
        if let Some((when, list)) = guard.as_ref() {
            if when.elapsed().as_secs() < TTL_SECS {
                let (exchanges, legacy) = summarize_entries(list);
                return json_ok(SymbolsResp {
                    count: list.len(),
                    exchanges,
                    entries: list.clone(),
                    symbols: legacy,
                });
            }
        }
    }
    // 2) 循环拉取所有注册交易所（各自失败互不影响）
    let mut entries: Vec<SymbolEntry> = Vec::new();
    for ex in Exchange::all() {
        match ctx.cache.list_symbols(ex) {
            Ok(list) => {
                log::info!("{} 交易对: {} 个", ex.as_str(), list.len());
                for s in &list {
                    entries.push(make_entry(ex, s));
                }
            }
            Err(e) => log::warn!("获取 {} 交易对失败: {}", ex.as_str(), e),
        }
    }

    if entries.is_empty() {
        return json_err(502, "symbols: 所有交易所均拉取失败".to_string());
    }

    if let Ok(mut guard) = ctx.symbols_cache.lock() {
        *guard = Some((std::time::Instant::now(), entries.clone()));
    }

    let (exchanges, legacy) = summarize_entries(&entries);
    json_ok(SymbolsResp {
        count: entries.len(),
        exchanges,
        entries,
        symbols: legacy,
    })
}

/// 根据 exchange + native_symbol 构造 SymbolEntry
/// 对于 USDT 交易对，base = symbol 去掉尾部 "USDT"
fn make_entry(ex: Exchange, native_symbol: &str) -> SymbolEntry {
    let base = native_symbol
        .strip_suffix("USDT")
        .unwrap_or(native_symbol)
        .to_string();
    SymbolEntry {
        id: format!("{}:{}", ex.as_str(), native_symbol),
        exchange: ex.as_str().to_string(),
        symbol: native_symbol.to_string(),
        base,
        quote: "USDT".to_string(),
    }
}

/// 从 entries 提取 (exchanges, legacy_binance_symbols)
fn summarize_entries(entries: &[SymbolEntry]) -> (Vec<String>, Vec<String>) {
    let mut exs = std::collections::BTreeSet::new();
    let mut legacy = Vec::new();
    for e in entries {
        exs.insert(e.exchange.clone());
        if e.exchange == "BINANCE" {
            legacy.push(e.symbol.clone());
        }
    }
    (exs.into_iter().collect(), legacy)
}

fn q<'a>(query: &'a HashMap<String, String>, key: &str) -> Option<&'a str> {
    query.get(key).map(|s| s.as_str())
}

fn parse_symbol(query: &HashMap<String, String>) -> Result<String, String> {
    q(query, "symbol")
        .filter(|s| !s.is_empty())
        .map(|s| s.to_uppercase())
        .ok_or_else(|| "缺少 symbol 参数，例如 symbol=BTCUSDT".to_string())
}

fn parse_timeframe(query: &HashMap<String, String>) -> Result<Timeframe, String> {
    let v = q(query, "interval").unwrap_or("4h");
    Timeframe::parse(v).ok_or_else(|| format!("非法 interval: {}", v))
}

fn parse_limit(query: &HashMap<String, String>, default: usize) -> usize {
    q(query, "limit")
        .and_then(|s| s.parse::<usize>().ok())
        .map(|n| n.clamp(20, 5000))
        .unwrap_or(default)
}

// --- /api/klines -------------------------------------------

#[derive(serde::Serialize)]
struct KlinesResp<'a> {
    symbol: String,
    interval: &'a str,
    count: usize,
    klines: &'a [crate::data::Kline],
}

fn handle_klines(
    ctx: &Ctx,
    query: &HashMap<String, String>,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let symbol = match parse_symbol(query) {
        Ok(v) => v,
        Err(e) => return json_err(400, e),
    };
    let tf = match parse_timeframe(query) {
        Ok(v) => v,
        Err(e) => return json_err(400, e),
    };
    let limit = parse_limit(query, 500);

    match ctx.cache.get(&symbol, tf, limit) {
        Ok(klines) => json_ok(KlinesResp {
            symbol: symbol.clone(),
            interval: tf.as_str(),
            count: klines.len(),
            klines: &klines.as_slice().to_vec(),
        }),
        Err(e) => json_err(502, format!("拉取 K线失败: {}", e)),
    }
}

// --- /api/ma_state -----------------------------------------

fn parse_periods(query: &HashMap<String, String>) -> Vec<usize> {
    if let Some(s) = q(query, "periods") {
        let mut v: Vec<usize> = s
            .split(',')
            .filter_map(|p| p.trim().parse::<usize>().ok())
            .filter(|&p| p >= 2 && p <= 1000)
            .collect();
        v.sort_unstable();
        v.dedup();
        if !v.is_empty() {
            return v;
        }
    }
    // 默认：在 PRD 基础上加入 MA30 作为主基准线
    vec![5, 10, 20, 30, 60, 120, 250]
}

fn parse_ma_kind(query: &HashMap<String, String>) -> MaKind {
    q(query, "kind")
        .and_then(MaKind::parse)
        .unwrap_or(MaKind::Sma)
}

fn handle_ma_state(
    ctx: &Ctx,
    query: &HashMap<String, String>,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let symbol = match parse_symbol(query) {
        Ok(v) => v,
        Err(e) => return json_err(400, e),
    };
    let tf = match parse_timeframe(query) {
        Ok(v) => v,
        Err(e) => return json_err(400, e),
    };
    let limit = parse_limit(query, 500);
    let periods = parse_periods(query);
    let kind = parse_ma_kind(query);

    let max_period = *periods.iter().max().unwrap_or(&20);
    let needed = limit.max(max_period + 60);
    let klines = match ctx.cache.get(&symbol, tf, needed) {
        Ok(v) => v,
        Err(e) => return json_err(502, format!("拉取 K线失败: {}", e)),
    };

    let state = ma::compute_ma_state(&symbol, tf.as_str(), kind, &klines, &periods);
    json_ok(state)
}

// --- /api/candle_patterns ----------------------------------

#[derive(serde::Serialize)]
struct CandlePatternsResp<'a> {
    symbol: String,
    interval: &'a str,
    klines_count: usize,
    pattern_count: usize,
    patterns: Vec<PatternHitOut>,
}

#[derive(serde::Serialize)]
struct PatternHitOut {
    index: usize,
    open_time: i64,
    code: String,
    label: &'static str,
    direction: i8,
    strength: u8,
}

fn handle_candle_patterns(
    ctx: &Ctx,
    query: &HashMap<String, String>,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let symbol = match parse_symbol(query) {
        Ok(v) => v,
        Err(e) => return json_err(400, e),
    };
    let tf = match parse_timeframe(query) {
        Ok(v) => v,
        Err(e) => return json_err(400, e),
    };
    let limit = parse_limit(query, 500);

    let klines = match ctx.cache.get(&symbol, tf, limit) {
        Ok(v) => v,
        Err(e) => return json_err(502, format!("拉取 K线失败: {}", e)),
    };
    let hits = candle::scan(&klines);
    let patterns: Vec<PatternHitOut> = hits
        .iter()
        .map(|h| PatternHitOut {
            index: h.index,
            open_time: klines.get(h.index).map(|k| k.open_time).unwrap_or(0),
            code: format!("{:?}", h.kind),
            label: h.kind.label(),
            direction: h.direction,
            strength: h.strength,
        })
        .collect();

    json_ok(CandlePatternsResp {
        symbol: symbol.clone(),
        interval: tf.as_str(),
        klines_count: klines.len(),
        pattern_count: patterns.len(),
        patterns,
    })
}

// --- /api/trend_state --------------------------------------

#[derive(serde::Serialize)]
struct TrendStateResp {
    symbol: String,
    interval: &'static str,
    state: trend::TrendState,
}

fn handle_trend_state(
    ctx: &Ctx,
    query: &HashMap<String, String>,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let symbol = match parse_symbol(query) {
        Ok(v) => v,
        Err(e) => return json_err(400, e),
    };
    let tf = match parse_timeframe(query) {
        Ok(v) => v,
        Err(e) => return json_err(400, e),
    };
    let limit = parse_limit(query, 500);
    let klines = match ctx.cache.get(&symbol, tf, limit) {
        Ok(v) => v,
        Err(e) => return json_err(502, format!("拉取 K线失败: {}", e)),
    };
    let state = trend::compute_trend_state(&klines);
    json_ok(TrendStateResp { symbol, interval: tf.as_str(), state })
}

// --- /api/chart_patterns -----------------------------------

#[derive(serde::Serialize)]
struct ChartPatternsResp {
    symbol: String,
    interval: &'static str,
    count: usize,
    patterns: Vec<chartpattern::ChartPattern>,
}

fn handle_chart_patterns(
    ctx: &Ctx,
    query: &HashMap<String, String>,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let symbol = match parse_symbol(query) {
        Ok(v) => v,
        Err(e) => return json_err(400, e),
    };
    let tf = match parse_timeframe(query) {
        Ok(v) => v,
        Err(e) => return json_err(400, e),
    };
    let limit = parse_limit(query, 500);
    let klines = match ctx.cache.get(&symbol, tf, limit) {
        Ok(v) => v,
        Err(e) => return json_err(502, format!("拉取 K线失败: {}", e)),
    };
    let patterns = chartpattern::detect_all(&klines);
    json_ok(ChartPatternsResp {
        symbol,
        interval: tf.as_str(),
        count: patterns.len(),
        patterns,
    })
}

// --- /api/resonance ----------------------------------------

#[derive(serde::Serialize)]
struct ResonanceResp {
    symbol: String,
    interval: &'static str,
    score: resonance::ResonanceScore,
    suggestion: resonance::TradeSuggestion,
    ma_specials: Vec<ma::MaSpecialHit>,
    current_price: f64,
    atr: f64,
    indicators: indicator::IndicatorSnapshot,
}

fn handle_resonance(
    ctx: &Ctx,
    query: &HashMap<String, String>,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let symbol = match parse_symbol(query) {
        Ok(v) => v,
        Err(e) => return json_err(400, e),
    };
    let tf = match parse_timeframe(query) {
        Ok(v) => v,
        Err(e) => return json_err(400, e),
    };
    let limit = parse_limit(query, 500);
    let periods = parse_periods(query);
    let ma_kind = parse_ma_kind(query);

    let klines = match ctx.cache.get(&symbol, tf, limit) {
        Ok(v) => v,
        Err(e) => return json_err(502, format!("拉取 K线失败: {}", e)),
    };
    if klines.is_empty() {
        return json_err(502, "无 K线数据");
    }

    // 均线状态 + 特殊形态
    let closes: Vec<f64> = klines.iter().map(|k| k.close).collect();
    let ma_state = ma::compute_ma_state(&symbol, tf.as_str(), ma_kind, &klines, &periods);
    let ma_series: Vec<Vec<f64>> = periods
        .iter()
        .map(|&p| ma::compute::compute(ma_kind, &closes, p))
        .collect();
    let cross_bars: Vec<usize> = ma_state.crosses.iter().map(|c| c.index).collect();
    // 基准均线：优先 MA30（PRD 升级），其次 MA20，最后退到第一条
    let base_period = if periods.contains(&30) { 30 }
        else if periods.contains(&20) { 20 }
        else { periods[0] };
    let base_idx = periods.iter().position(|&p| p == base_period).unwrap_or(0);
    let base_ma = &ma_series[base_idx];
    let slope_series = ma::compute::slope(base_ma, 5);
    let bar_idx = klines.len() - 1;
    let specials = ma::scan_ma_special(
        &closes,
        &ma_series,
        &periods,
        ma_state.alignment,
        &slope_series,
        base_period,
        &cross_bars,
        bar_idx,
        &ma::SpecialParams::default(),
    );

    // 趋势 / K线 / 技术图形
    let trend_state = trend::compute_trend_state(&klines);
    let candles = candle::scan(&klines);
    let charts = chartpattern::detect_all(&klines);

    // 共振评分（允许从 query 覆盖权重）
    let mut weights = resonance::score::ResonanceWeights::default();
    if let Some(v) = q(query, "w_ma").and_then(|s| s.parse::<f64>().ok()) { weights.ma = v; }
    if let Some(v) = q(query, "w_trend").and_then(|s| s.parse::<f64>().ok()) { weights.trend = v; }
    if let Some(v) = q(query, "w_candle").and_then(|s| s.parse::<f64>().ok()) { weights.candle = v; }
    if let Some(v) = q(query, "w_chart").and_then(|s| s.parse::<f64>().ok()) { weights.chart = v; }
    let score = resonance::compute_resonance(
        &klines, &ma_state, &specials, &trend_state, &candles, &charts, &weights,
    );

    // ATR 计算
    let atr_series = trend::swing::atr_series(&klines, 14);
    let atr = atr_series.last().copied().unwrap_or(0.0);
    let current_price = klines.last().unwrap().close;

    // 建议计算
    let mut sug_input = resonance::suggestion::SuggestionInput::default();
    sug_input.current_price = current_price;
    sug_input.atr = atr;
    if let Some(eq) = q(query, "equity").and_then(|s| s.parse::<f64>().ok()) { sug_input.account_equity = eq; }
    if let Some(r) = q(query, "max_risk").and_then(|s| s.parse::<f64>().ok()) { sug_input.max_risk_pct = r; }
    if let Some(rr) = q(query, "rr").and_then(|s| s.parse::<f64>().ok()) { sug_input.rr_target = rr; }
    if let Some(m) = q(query, "atr_mult").and_then(|s| s.parse::<f64>().ok()) { sug_input.atr_stop_mult = m; }
    let suggestion = resonance::compute_suggestion(&score, &sug_input);

    let indicators = indicator::compute_snapshot(&klines);

    // Sprint B 3/3：触发实时增量学习（后台异步，不阻塞响应）
    maybe_trigger_live_learn(ctx, &symbol, tf, limit);

    json_ok(ResonanceResp {
        symbol,
        interval: tf.as_str(),
        score,
        suggestion,
        ma_specials: specials,
        current_price,
        atr,
        indicators,
    })
}

// --- /api/backtest/run -------------------------------------

fn parse_f64(query: &HashMap<String, String>, key: &str, default: f64) -> f64 {
    q(query, key).and_then(|s| s.parse::<f64>().ok()).unwrap_or(default)
}

fn parse_stop_kind(query: &HashMap<String, String>) -> StopKind {
    match q(query, "stop_kind").unwrap_or("atr").to_ascii_lowercase().as_str() {
        "structure" => StopKind::Structure,
        "ma" => StopKind::Ma,
        "pattern" => StopKind::Pattern,
        _ => StopKind::Atr,
    }
}

fn handle_backtest(
    ctx: &Ctx,
    query: &HashMap<String, String>,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let symbol = match parse_symbol(query) {
        Ok(v) => v,
        Err(e) => return json_err(400, e),
    };
    let tf = match parse_timeframe(query) {
        Ok(v) => v,
        Err(e) => return json_err(400, e),
    };
    let limit = parse_limit(query, 1000);
    let periods = parse_periods(query);
    let ma_kind = parse_ma_kind(query);
    let stop_kind = parse_stop_kind(query);

    let base_period = q(query, "base_period")
        .and_then(|s| s.parse::<usize>().ok())
        .filter(|p| periods.contains(p))
        .unwrap_or_else(|| if periods.contains(&30) { 30 }
            else if periods.contains(&20) { 20 }
            else { periods[0] });

    let cfg = BacktestConfig {
        symbol: symbol.clone(),
        interval: tf.as_str().to_string(),
        limit,
        initial_capital: parse_f64(query, "capital", 10_000.0),
        risk_per_trade: parse_f64(query, "risk", 0.02),
        rr_ratio: parse_f64(query, "rr", 2.0),
        stop_kind,
        atr_multiplier: parse_f64(query, "atr_mult", 1.5),
        fee_bps: parse_f64(query, "fee_bps", 5.0),
        slippage_bps: parse_f64(query, "slip_bps", 5.0),
        ma_kind,
        ma_periods: periods,
        base_period,
        min_pattern_strength: q(query, "min_strength")
            .and_then(|s| s.parse::<u8>().ok())
            .unwrap_or(4),
        allow_short: q(query, "allow_short")
            .map(|s| !matches!(s, "0" | "false" | "no"))
            .unwrap_or(true),
    };

    let klines = match ctx.cache.get(&symbol, tf, limit) {
        Ok(v) => v,
        Err(e) => return json_err(502, format!("拉取 K线失败: {}", e)),
    };

    log::info!(
        "回测 {} {} bars={} capital={} risk={}",
        cfg.symbol, cfg.interval, klines.len(), cfg.initial_capital, cfg.risk_per_trade
    );
    let result = backtest::run(&klines, &cfg);
    json_ok(result)
}

// --- /api/signals ----------------------------------------
//
// Sprint 9 UI 集成：暴露 Sprint 3-7 所有核心新信号
//
// 返回：
// - 多合一合流（R-P1-16）
// - ma 高级形态（旱地拔葱/毒蜘蛛/断头铡刀/向上发散）
// - 旗形 7 条验证结果（对 detect 出的旗形附加验证）
// - 排列状态（多头/空头/无）
// - 收敛发散状态

#[derive(serde::Serialize)]
struct SignalsResp {
    symbol: String,
    interval: &'static str,
    bars: usize,
    current_alignment: String,
    ma_relation: String,
    confluences: Vec<signal::Confluence>,
    advanced_ma_events: Vec<ma::MaAdvancedEvent>,
    flag_validations: Vec<FlagValidationItem>,
    bull_traps: Vec<signal::TrapEvent>,
    stealth_breakouts: Vec<signal::StealthBreakoutEvent>,
    // Sprint 17：新增字段
    volume_anomalies: Vec<signal::VolumeAnomalyEvent>,
    long_term_hits: Vec<ma::LongTermLevelHit>,
    trend_transitions: Vec<crate::engine::trend::state_machine::TransitionRecord>,
    candle_combinations: Vec<crate::engine::candle::combinations::CombinationEvent>,
}

#[derive(serde::Serialize)]
struct FlagValidationItem {
    pattern: chartpattern::ChartPattern,
    validation: chartpattern::FlagValidation,
}

fn handle_signals(
    ctx: &Ctx,
    query: &HashMap<String, String>,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let symbol = match parse_symbol(query) {
        Ok(v) => v,
        Err(e) => return json_err(400, e),
    };
    let tf = match parse_timeframe(query) {
        Ok(v) => v,
        Err(e) => return json_err(400, e),
    };
    let limit = parse_limit(query, 500);
    let klines = match ctx.cache.get(&symbol, tf, limit) {
        Ok(v) => v,
        Err(e) => return json_err(502, format!("拉取 K线失败: {}", e)),
    };
    if klines.len() < 100 {
        return json_err(400, "K 线数量不足 100 根，无法识别高级信号");
    }
    let closes: Vec<f64> = klines.iter().map(|k| k.close).collect();
    let opens: Vec<f64> = klines.iter().map(|k| k.open).collect();
    let volumes: Vec<f64> = klines.iter().map(|k| k.volume).collect();

    // 1. 多合一识别（用 MA + 前高作为组件代理）
    let periods = [5, 10, 20, 60, 120];
    let mas_series: Vec<Vec<f64>> = periods
        .iter()
        .map(|&p| ma::sma(&closes, p))
        .collect();
    let last = closes.len() - 1;
    let mut components: Vec<signal::ConfluenceComponent> = Vec::new();
    for (j, p) in periods.iter().enumerate() {
        let v = mas_series[j][last];
        if v.is_finite() {
            components.push(signal::ConfluenceComponent::MovingAverage {
                period: *p,
                price: v,
            });
        }
    }
    if last >= 60 {
        let phi = klines[last - 60..last]
            .iter()
            .map(|k| k.high)
            .fold(f64::NEG_INFINITY, f64::max);
        let plo = klines[last - 60..last]
            .iter()
            .map(|k| k.low)
            .fold(f64::INFINITY, f64::min);
        components.push(signal::ConfluenceComponent::PriorSwingPoint {
            is_high: true,
            price: phi,
        });
        components.push(signal::ConfluenceComponent::PriorSwingPoint {
            is_high: false,
            price: plo,
        });
    }
    let confluences =
        signal::detect_confluences(&components, &signal::ConfluenceParams::default());

    // 2. ma 高级形态
    let adv_mas: Vec<Vec<f64>> = [5, 10, 20, 60]
        .iter()
        .map(|&p| ma::sma(&closes, p))
        .collect();
    let adv_periods = vec![5usize, 10, 20, 60];
    let adv_events = ma::scan_advanced(
        &closes,
        &opens,
        &volumes,
        &adv_mas,
        &adv_periods,
        &ma::MaAdvancedParams::default(),
    );

    // 3. 旗形验证
    let chart_patterns = chartpattern::detect_all(&klines);
    let mut flag_vals: Vec<FlagValidationItem> = Vec::new();
    for p in &chart_patterns {
        if matches!(
            p.kind,
            chartpattern::ChartPatternKind::BullFlag | chartpattern::ChartPatternKind::BearFlag
        ) {
            if let Some(v) = chartpattern::validate_flag(
                p,
                &klines,
                &chartpattern::FlagValidatorParams::default(),
            ) {
                flag_vals.push(FlagValidationItem {
                    pattern: p.clone(),
                    validation: v,
                });
            }
        }
    }

    // 4. 多头/空头陷阱（用每根 K 线对应的 MA60 作为 key_price，避免未来函数）
    let traps = signal::detect_traps_with_key_series(
        &closes,
        &mas_series[3],
        &signal::TrapParams::default(),
    );

    // 5. 主力潜伏突破
    let stealth_events = signal::detect_stealth_breakouts(
        &opens,
        &closes,
        &volumes,
        &signal::StealthParams::default(),
    );

    // 6. 当前排列 + 收敛发散
    let align_mas_now: Vec<f64> = adv_mas.iter().map(|m| m[last]).collect();
    let align_mas_back_idx = last.saturating_sub(5);
    let align_mas_back: Vec<f64> =
        adv_mas.iter().map(|m| m[align_mas_back_idx]).collect();
    let alignment = candle::detect_alignment(closes[last], &align_mas_now, &align_mas_back);
    let relation = candle::detect_ma_relation(&align_mas_now, &align_mas_back, 0.015);

    // Sprint 17：新识别器集成
    // 7. 无量跌停/涨停警告（R-P1-26）
    let volume_anomalies = signal::detect_volume_anomalies(
        &klines,
        &signal::VolumeWarningParams::default(),
    );

    // 8. 120/240 日长期压力位（R-P1-29）
    let long_term_hits =
        ma::scan_long_term_levels(&closes, &ma::LongTermParams::default());

    // 9. 趋势状态机转移记录（R-P1-08）
    let trend_state = crate::engine::trend::compute_trend_state(&klines);
    let mut sm = crate::engine::trend::state_machine::TrendStateMachine::new();
    sm.update(&trend_state.swings, klines.len().saturating_sub(1));
    let trend_transitions = sm.history().to_vec();

    // 10. K 线组合（R-P1-09）
    let candle_hits = candle::scan(&klines);
    let candle_combinations =
        crate::engine::candle::combinations::detect_combinations(&candle_hits);

    // Sprint B 3/3：触发实时增量学习
    maybe_trigger_live_learn(ctx, &symbol, tf, limit);

    json_ok(SignalsResp {
        symbol,
        interval: tf.as_str(),
        bars: klines.len(),
        current_alignment: alignment.label().to_string(),
        ma_relation: relation.label().to_string(),
        confluences,
        advanced_ma_events: adv_events,
        flag_validations: flag_vals,
        bull_traps: traps,
        stealth_breakouts: stealth_events,
        volume_anomalies,
        long_term_hits,
        trend_transitions,
        candle_combinations,
    })
}

// --- /api/backtest/playbook ----------------------------------------
//
// Sprint 12：Playbook 驱动的回测端点
//
// 使用 CompositePlaybook 默认组合（断头铡刀清仓 > 三次减仓 > 趋势矩阵 > 旱地拔葱）
//
// 参数：symbol, interval, limit, strategy（可选："default"/"guillotine"/"scallions"/"staged_exit"/"trend_matrix"）
//
// 返回：BacktestResult（复用现有结构）+ strategy_name + book_source

#[derive(serde::Serialize)]
struct PlaybookBacktestResp {
    strategy_name: String,
    book_source: String,
    result: backtest::BacktestResult,
}

fn handle_playbook_backtest(
    ctx: &Ctx,
    query: &HashMap<String, String>,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let symbol = match parse_symbol(query) {
        Ok(v) => v,
        Err(e) => return json_err(400, e),
    };
    let tf = match parse_timeframe(query) {
        Ok(v) => v,
        Err(e) => return json_err(400, e),
    };
    let limit = parse_limit(query, 1000);
    let strategy = query
        .get("strategy")
        .map(|s| s.as_str())
        .unwrap_or("default");

    let klines = match ctx.cache.get(&symbol, tf, limit) {
        Ok(v) => v,
        Err(e) => return json_err(502, format!("拉取 K线失败: {}", e)),
    };

    let mut cfg = backtest::BacktestConfig::default();
    cfg.symbol = symbol.clone();
    cfg.interval = tf.as_str().to_string();
    cfg.limit = limit;

    let (name, src): (String, String);
    let result = match strategy {
        "guillotine" => {
            name = "断头铡刀清仓".into();
            src = "ma p.380".into();
            let mut pb = backtest::GuillotineExitPlaybook;
            backtest::run_with_playbook(&klines, &cfg, &mut pb)
        }
        "scallions" => {
            name = "旱地拔葱轻仓入场".into();
            src = "ma p.340".into();
            let mut pb = backtest::HangingScallionsEntryPlaybook;
            backtest::run_with_playbook(&klines, &cfg, &mut pb)
        }
        "staged_exit" => {
            name = "三次减仓".into();
            src = "candle p.605".into();
            let mut pb = backtest::StagedExitPlaybook::new();
            backtest::run_with_playbook(&klines, &cfg, &mut pb)
        }
        "trend_matrix" => {
            name = "多级趋势线矩阵".into();
            src = "trend p.216".into();
            let mut pb = backtest::TrendMatrixPlaybook;
            backtest::run_with_playbook(&klines, &cfg, &mut pb)
        }
        _ => {
            name = "组合策略（默认）".into();
            src = "三书综合".into();
            let mut pb = backtest::CompositePlaybook::default_combo();
            backtest::run_with_playbook(&klines, &cfg, &mut pb)
        }
    };

    log::info!(
        "Playbook 回测 {} {} bars={} strategy={} 收益={:.2}%",
        symbol,
        tf.as_str(),
        klines.len(),
        strategy,
        result.performance.total_return_pct
    );

    json_ok(PlaybookBacktestResp {
        strategy_name: name,
        book_source: src,
        result,
    })
}

// --- /api/decision ----------------------------------------
//
// UI P0-1：聚合当前所有信号 → 生成"现在应该做什么"决策
//
// 返回：
// - action: buy/sell/hold/watch
// - confidence: 0-100
// - risk_level: low/medium/high
// - reasons: 3-5 条原书依据
// - suggested_actions: 按钮配置
// - book_sources: 引用的原书页码

#[derive(serde::Serialize)]
struct DecisionResp {
    symbol: String,
    interval: &'static str,
    current_price: f64,
    action: &'static str,
    action_label: &'static str,
    confidence: u8,
    risk_level: &'static str,
    risk_label: &'static str,
    reasons: Vec<String>,
    suggested_actions: Vec<SuggestedAction>,
    book_sources: Vec<String>,
}

#[derive(serde::Serialize)]
struct SuggestedAction {
    label: String,
    kind: &'static str, // "primary" | "secondary" | "danger"
    hint: String,
}

fn handle_decision(
    ctx: &Ctx,
    query: &HashMap<String, String>,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let symbol = match parse_symbol(query) {
        Ok(v) => v,
        Err(e) => return json_err(400, e),
    };
    let tf = match parse_timeframe(query) {
        Ok(v) => v,
        Err(e) => return json_err(400, e),
    };
    let limit = parse_limit(query, 500);
    let klines = match ctx.cache.get(&symbol, tf, limit) {
        Ok(v) => v,
        Err(e) => return json_err(502, format!("拉取 K线失败: {}", e)),
    };
    if klines.len() < 100 {
        return json_err(400, "K 线不足");
    }
    let closes: Vec<f64> = klines.iter().map(|k| k.close).collect();
    let opens: Vec<f64> = klines.iter().map(|k| k.open).collect();
    let volumes: Vec<f64> = klines.iter().map(|k| k.volume).collect();
    let last = closes.len() - 1;
    let current_price = closes[last];

    // 1. 扫描 ma 高级事件
    let periods = [5usize, 10, 20, 60];
    let mas: Vec<Vec<f64>> = periods.iter().map(|&p| ma::sma(&closes, p)).collect();
    let adv_events = ma::scan_advanced(
        &closes,
        &opens,
        &volumes,
        &mas,
        &[5usize, 10, 20, 60],
        &ma::MaAdvancedParams::default(),
    );
    // 最近的高级信号
    let recent_adv = adv_events
        .iter()
        .rev()
        .find(|e| last.saturating_sub(e.index) < 10);

    // 2. 道氏趋势
    let trend_state = crate::engine::trend::compute_trend_state(&klines);
    let dow_phase = trend_state.dow.phase;
    let long_trend: i8 = match dow_phase {
        crate::engine::trend::DowPhase::Uptrend => 1,
        crate::engine::trend::DowPhase::Downtrend => -1,
        _ => 0,
    };

    // 3. 多合一
    let mut components: Vec<signal::ConfluenceComponent> = Vec::new();
    for (j, p) in periods.iter().enumerate() {
        let v = mas[j][last];
        if v.is_finite() {
            components.push(signal::ConfluenceComponent::MovingAverage {
                period: *p,
                price: v,
            });
        }
    }
    let confluences =
        signal::detect_confluences(&components, &signal::ConfluenceParams::default());
    let strong_confluence = confluences.iter().any(|c| c.unique_kinds >= 3);

    // 4. L4 警告（如有最近葛南维信号）
    let mut l4_warning = signal::L4WarningLevel::None;
    if let Some(adv) = recent_adv {
        l4_warning = signal::detect_l4_warning(
            ma::GranvilleRule::B4DivergenceBuy,
            Some(adv.kind),
            dow_phase,
        );
    }

    // 5. 构造 Playbook 决策
    let mut playbook = backtest::CompositePlaybook::default_combo();
    let topping = recent_adv.and_then(|e| match e.kind {
        ma::MaAdvancedKind::Guillotine => {
            Some(signal::ToppingSignalSeverity::Severe)
        }
        ma::MaAdvancedKind::PoissonSpider => {
            Some(signal::ToppingSignalSeverity::Intermediate)
        }
        _ => None,
    });
    let ctx_pb = backtest::PlaybookContext {
        klines: &klines,
        index: last,
        current_position: 0.5, // 假设持仓 50%
        ma_advanced_kind: recent_adv.map(|e| e.kind),
        topping_severity: topping,
        long_trend,
    };
    let decision = playbook.decide(&ctx_pb);

    // 6. 映射为响应
    let (action, action_label, confidence, actions): (
        &'static str,
        &'static str,
        u8,
        Vec<SuggestedAction>,
    ) = match &decision {
        backtest::PlaybookDecision::Buy {
            target_position, ..
        } => (
            "buy",
            "建议买入",
            70,
            vec![
                SuggestedAction {
                    label: format!("买入至 {:.0}%", target_position * 100.0),
                    kind: "primary",
                    hint: "按建议仓位建仓".to_string(),
                },
                SuggestedAction {
                    label: "观望".to_string(),
                    kind: "secondary",
                    hint: "等待更多确认".to_string(),
                },
            ],
        ),
        backtest::PlaybookDecision::Sell {
            target_position, ..
        } => {
            let is_full = *target_position < 1e-9_f64;
            let target_position = *target_position;
            (
                "sell",
                if is_full { "建议清仓" } else { "建议减仓" },
                if is_full { 90 } else { 75 },
                vec![
                    SuggestedAction {
                        label: if is_full {
                            "清仓".to_string()
                        } else {
                            format!("减仓至 {:.0}%", target_position * 100.0)
                        },
                        kind: "danger",
                        hint: "立即执行".to_string(),
                    },
                    SuggestedAction {
                        label: "持股观察".to_string(),
                        kind: "secondary",
                        hint: "可能错过最佳离场".to_string(),
                    },
                ],
            )
        }
        backtest::PlaybookDecision::StayOut { .. } => (
            "watch",
            "空仓观望",
            60,
            vec![SuggestedAction {
                label: "保持空仓".to_string(),
                kind: "secondary",
                hint: "长期下降趋势非牛市空仓".to_string(),
            }],
        ),
        backtest::PlaybookDecision::Hold => (
            "hold",
            "持有不动",
            50,
            vec![SuggestedAction {
                label: "持仓观察".to_string(),
                kind: "secondary",
                hint: "暂无明确信号".to_string(),
            }],
        ),
    };

    // 7. 生成原书依据的 reasons
    let mut reasons = Vec::new();
    let mut book_sources: Vec<String> = Vec::new();

    // 长期趋势
    match dow_phase {
        crate::engine::trend::DowPhase::Uptrend => {
            reasons.push("✅ 长期上升趋势（道氏 HH/HL）".to_string());
        }
        crate::engine::trend::DowPhase::Downtrend => {
            reasons.push("⚠️ 长期下降趋势（道氏 LL/LH）".to_string());
            book_sources.push("trend p.225（非牛市空仓）".to_string());
        }
        crate::engine::trend::DowPhase::Consolidation => {
            reasons.push("➖ 整固整理".to_string());
        }
        _ => {}
    }

    // ma 高级信号
    if let Some(ev) = recent_adv {
        let label = match ev.kind {
            ma::MaAdvancedKind::Guillotine => "⚠️ 断头铡刀触发（最强空头）",
            ma::MaAdvancedKind::PoissonSpider => "⚠️ 毒蜘蛛（首次死叉）",
            ma::MaAdvancedKind::HangingScallions => "🌱 旱地拔葱（早期看涨）",
            ma::MaAdvancedKind::BondUpwardDiverge => "🚀 主升浪启动信号",
        };
        reasons.push(label.to_string());
        book_sources.push(format!("ma {}", ev.kind.book_source()));
    }

    // 多合一
    if strong_confluence {
        reasons.push(format!(
            "🎯 多合一共振（{} 类组件）",
            confluences.iter().map(|c| c.unique_kinds).max().unwrap_or(0)
        ));
        book_sources.push("trend p.216 多合一现象".to_string());
    }

    // L4 警告
    match l4_warning {
        signal::L4WarningLevel::Critical => {
            reasons.push("🚨 L4 共振警告：严重（应反向卖出）".to_string());
            book_sources.push("ma p.100 葛南维 L4".to_string());
        }
        signal::L4WarningLevel::Caution => {
            reasons.push("⚠️ L4 共振警告：轻仓".to_string());
            book_sources.push("ma p.100 葛南维 L4".to_string());
        }
        _ => {}
    }

    // 当前价 vs 60 日
    let ma60 = mas[3][last];
    if ma60.is_finite() {
        let diff = (current_price - ma60) / ma60 * 100.0;
        if diff.abs() < 1.0 {
            reasons.push(format!("➖ 价格触及 MA60 支撑/压力 ({:.2})", ma60));
        } else if diff > 5.0 {
            reasons.push(format!(
                "📈 价格高于 MA60 约 {:.1}%（偏离预警）",
                diff
            ));
        } else if diff < -5.0 {
            reasons.push(format!(
                "📉 价格低于 MA60 约 {:.1}%（修复机会）",
                diff
            ));
        }
    }

    // 风险等级
    let risk_level: &'static str = match (&decision, &l4_warning) {
        (_, signal::L4WarningLevel::Critical) => "high",
        (backtest::PlaybookDecision::Sell { .. }, _) => "medium",
        (backtest::PlaybookDecision::StayOut { .. }, _) => "medium",
        _ => {
            if dow_phase == crate::engine::trend::DowPhase::Downtrend {
                "high"
            } else if strong_confluence {
                "low"
            } else {
                "medium"
            }
        }
    };
    let risk_label: &'static str = match risk_level {
        "high" => "高",
        "medium" => "中",
        "low" => "低",
        _ => "—",
    };

    // 确保 reasons 非空
    if reasons.is_empty() {
        reasons.push("当前无明显信号，继续观察".to_string());
    }

    // Sprint B 3/3：触发实时增量学习
    maybe_trigger_live_learn(ctx, &symbol, tf, limit);

    json_ok(DecisionResp {
        symbol,
        interval: tf.as_str(),
        current_price,
        action,
        action_label,
        confidence,
        risk_level,
        risk_label,
        reasons,
        suggested_actions: actions,
        book_sources,
    })
}

// --- /api/indicators/series -------------------------------
//
// 返回 RSI / MACD 等常用指标的完整时序（与 /api/klines 的 open_time 对齐）
//
// 参数：
//   symbol, interval, limit
//   kinds=rsi,macd (默认)   可选值：rsi / macd / volume_ma
//   rsi_period=14    macd_fast=12 macd_slow=26 macd_signal=9
//
// 返回：
//   { times: [ms], rsi: Option<[f64]>, macd: Option<{line, signal, hist}>,
//     volume_ma: Option<[f64]>, volume: Option<[f64]> }

#[derive(serde::Serialize)]
struct MacdSeries {
    line: Vec<f64>,
    signal: Vec<f64>,
    hist: Vec<f64>,
}

#[derive(serde::Serialize)]
struct StochRsiSeries {
    k: Vec<f64>,
    d: Vec<f64>,
}

#[derive(serde::Serialize)]
struct IndicatorsSeriesResp {
    symbol: String,
    interval: &'static str,
    times: Vec<i64>, // ms
    #[serde(skip_serializing_if = "Option::is_none")]
    rsi: Option<Vec<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    macd: Option<MacdSeries>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stoch_rsi: Option<StochRsiSeries>,
    #[serde(skip_serializing_if = "Option::is_none")]
    volume: Option<Vec<f64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    volume_ma: Option<Vec<f64>>,
}

fn handle_indicators_series(
    ctx: &Ctx,
    query: &HashMap<String, String>,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let symbol = match parse_symbol(query) {
        Ok(v) => v,
        Err(e) => return json_err(400, e),
    };
    let tf = match parse_timeframe(query) {
        Ok(v) => v,
        Err(e) => return json_err(400, e),
    };
    let limit = parse_limit(query, 500);
    let klines = match ctx.cache.get(&symbol, tf, limit) {
        Ok(v) => v,
        Err(e) => return json_err(502, format!("拉取 K线失败: {}", e)),
    };
    if klines.is_empty() {
        return json_err(400, "K 线为空");
    }

    // 解析 kinds（逗号分隔）；缺省同时返回 rsi 和 macd
    let kinds: Vec<String> = q(query, "kinds")
        .map(|s| {
            s.split(',')
                .map(|x| x.trim().to_lowercase())
                .filter(|x| !x.is_empty())
                .collect()
        })
        .unwrap_or_else(|| vec!["rsi".to_string(), "macd".to_string()]);

    let closes: Vec<f64> = klines.iter().map(|k| k.close).collect();
    let volumes: Vec<f64> = klines.iter().map(|k| k.volume).collect();
    let times: Vec<i64> = klines.iter().map(|k| k.open_time).collect();

    let mut rsi_out: Option<Vec<f64>> = None;
    let mut macd_out: Option<MacdSeries> = None;
    let mut stoch_rsi_out: Option<StochRsiSeries> = None;
    let mut vol_out: Option<Vec<f64>> = None;
    let mut vol_ma_out: Option<Vec<f64>> = None;

    if kinds.iter().any(|s| s == "rsi") {
        let period = q(query, "rsi_period")
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(14);
        rsi_out = Some(indicator::rsi(&closes, period));
    }

    if kinds.iter().any(|s| s == "stoch_rsi" || s == "stochrsi") {
        let rsi_p = q(query, "stoch_rsi_period")
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(14);
        let stoch_p = q(query, "stoch_period")
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(14);
        let k_smooth = q(query, "stoch_k_smooth")
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(3);
        let d_smooth = q(query, "stoch_d_smooth")
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(3);
        let (k, d) = indicator::stoch_rsi(&closes, rsi_p, stoch_p, k_smooth, d_smooth);
        stoch_rsi_out = Some(StochRsiSeries { k, d });
    }

    if kinds.iter().any(|s| s == "macd") {
        let fast = q(query, "macd_fast")
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(12);
        let slow = q(query, "macd_slow")
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(26);
        let signal = q(query, "macd_signal")
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(9);
        let (line, sig) = indicator::macd(&closes, fast, slow, signal);
        let hist: Vec<f64> = line
            .iter()
            .zip(sig.iter())
            .map(|(l, s)| {
                if l.is_finite() && s.is_finite() {
                    l - s
                } else {
                    f64::NAN
                }
            })
            .collect();
        macd_out = Some(MacdSeries {
            line,
            signal: sig,
            hist,
        });
    }

    if kinds.iter().any(|s| s == "volume" || s == "volume_ma") {
        vol_out = Some(volumes.clone());
        // 20 周期均量
        let mut ma = vec![f64::NAN; volumes.len()];
        let window = 20usize;
        if volumes.len() >= window {
            let mut sum: f64 = volumes[..window].iter().sum();
            ma[window - 1] = sum / window as f64;
            for i in window..volumes.len() {
                sum += volumes[i] - volumes[i - window];
                ma[i] = sum / window as f64;
            }
        }
        vol_ma_out = Some(ma);
    }

    json_ok(IndicatorsSeriesResp {
        symbol,
        interval: tf.as_str(),
        times,
        rsi: rsi_out,
        macd: macd_out,
        stoch_rsi: stoch_rsi_out,
        volume: vol_out,
        volume_ma: vol_ma_out,
    })
}

// --- /api/effectiveness ---------------------------------
//
// Sprint A：指标有效性评估（离线统计）
//
// 参数：
//   symbol, interval, limit   K 线来源（复用现有 cache）
//   horizon=10                结算窗口（几根 K 线后评估方向是否正确）
//
// 返回：
//   { symbol, interval, bars, horizon, total_triggers,
//     rankings: [EffectivenessEntry…] 按综合评分降序 }

fn handle_effectiveness(
    ctx: &Ctx,
    query: &HashMap<String, String>,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let symbol = match parse_symbol(query) {
        Ok(v) => v,
        Err(e) => return json_err(400, e),
    };
    let tf = match parse_timeframe(query) {
        Ok(v) => v,
        Err(e) => return json_err(400, e),
    };
    let limit = parse_limit(query, 2000);
    let horizon = q(query, "horizon")
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(effectiveness::DEFAULT_HORIZON);

    let klines = match ctx.cache.get(&symbol, tf, limit) {
        Ok(v) => v,
        Err(e) => return json_err(502, format!("拉取 K线失败: {}", e)),
    };

    let report = effectiveness::evaluate(&klines, &symbol, tf.as_str(), horizon);
    log::info!(
        "effectiveness {} {} bars={} horizon={} triggers={} arms={}",
        symbol,
        tf.as_str(),
        report.bars,
        horizon,
        report.total_triggers,
        report.rankings.len()
    );
    json_ok(report)
}

// --- /api/bandit/state ----------------------------------
//
// Sprint B：返回当前 Bandit 的所有 arm 后验 + 元数据
//
// 响应示例：
// {
//   "version": 1,
//   "total_plays": 1234,
//   "total_settled": 987,
//   "pending": 12,
//   "arms": [ArmState, ...]  // 按 posterior_mean 降序
// }

#[derive(serde::Serialize)]
struct BanditStateResp {
    version: u32,
    total_plays: u64,
    total_settled: u64,
    pending: usize,
    last_saved_ms: i64,
    arms: Vec<rl::ArmState>,
}

fn handle_bandit_state(ctx: &Ctx) -> Response<std::io::Cursor<Vec<u8>>> {
    let guard = match ctx.bandit.lock() {
        Ok(g) => g,
        Err(e) => return json_err(500, format!("bandit lock poisoned: {}", e)),
    };
    let arms = rl::rank_snapshot(&guard);
    json_ok(BanditStateResp {
        version: guard.version,
        total_plays: guard.total_plays,
        total_settled: guard.total_settled,
        pending: guard.pending.len(),
        last_saved_ms: guard.last_saved_ms,
        arms,
    })
}

// --- /api/bandit/train ----------------------------------
//
// Sprint B：离线训练端点（在历史 K 线上回放 effectiveness 的所有触发点，
// 按 Thompson Sampling 抽样选中某一 arm 并结算到 Bandit state；
// 保存到磁盘）
//
// 参数：symbol, interval, limit, horizon
//   policy=thompson|ucb1|greedy（默认 thompson）
//   min_samples=5（冷启动保护）
//
// 响应：{ before: u64, after: u64, settled: u64, arms: Vec<ArmState> }
//
// 这是 Sprint B 从 Sprint A 平滑升级到 Bandit 的最小代价路径：
// 不需要改变现有用户操作流，在服务启动时就可以一键回放几年数据给 bandit"读书"。

#[derive(serde::Serialize)]
struct BanditTrainResp {
    symbol: String,
    interval: &'static str,
    bars: usize,
    horizon: usize,
    triggers_scanned: usize,
    arms_updated: usize,
    policy: &'static str,
    before_plays: u64,
    after_plays: u64,
    after_settled: u64,
    arms: Vec<rl::ArmState>,
}

fn handle_bandit_train(
    ctx: &Ctx,
    query: &HashMap<String, String>,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let symbol = match parse_symbol(query) {
        Ok(v) => v,
        Err(e) => return json_err(400, e),
    };
    let tf = match parse_timeframe(query) {
        Ok(v) => v,
        Err(e) => return json_err(400, e),
    };
    let limit = parse_limit(query, 2000);
    let horizon = q(query, "horizon")
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(effectiveness::DEFAULT_HORIZON);

    let policy = q(query, "policy").unwrap_or("thompson");
    let (policy_enum, policy_label): (rl::SelectionPolicy, &'static str) = match policy {
        "ucb1" => (rl::SelectionPolicy::Ucb1 { c: 2 }, "ucb1"),
        "greedy" => (rl::SelectionPolicy::Greedy, "greedy"),
        _ => (rl::SelectionPolicy::Thompson, "thompson"),
    };

    let min_samples = q(query, "min_samples")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(5);

    let klines = match ctx.cache.get(&symbol, tf, limit) {
        Ok(v) => v,
        Err(e) => return json_err(502, format!("拉取 K线失败: {}", e)),
    };

    // 1. 扫描触发点（复用 Sprint A 的评估器，但这里直接跑一次 evaluate 拿全部 arm 的 n 分布
    //    → 实际训练：逐 bar 重放，每个触发点直接进入 bandit.register_trigger + on_new_bar）
    //
    // 为简单起见：我们只把 Sprint A 的 rankings 当作训练数据源，
    // 对每个 EffectivenessEntry 构造一个 PendingEvaluation 逐条 settle。
    //
    // 这跟真正在线的触发顺序略有差异（丢失了时间顺序交错），
    // 但对 Beta 后验更新是等价的（累加 α/β 顺序无关）。

    let report = effectiveness::evaluate(&klines, &symbol, tf.as_str(), horizon);

    let mut guard = match ctx.bandit.lock() {
        Ok(g) => g,
        Err(e) => return json_err(500, format!("bandit lock poisoned: {}", e)),
    };
    let before = guard.total_plays;
    let now_ms = unix_now_ms();
    let arms_updated = rl::merge_report(&mut guard, &report, now_ms);
    let _ = (policy_enum, min_samples); // 保留参数以便未来 online 版本

    if let Err(e) = rl::save(&ctx.cfg.cache_dir, &guard) {
        log::warn!("Bandit state save failed: {}", e);
    } else {
        log::info!(
            "Bandit trained from {} {}: {} arms updated, total_plays {} → {}",
            symbol,
            tf.as_str(),
            arms_updated,
            before,
            guard.total_plays
        );
    }

    let after_plays = guard.total_plays;
    let after_settled = guard.total_settled;
    let arms_out = rl::rank_snapshot(&guard);

    json_ok(BanditTrainResp {
        symbol,
        interval: tf.as_str(),
        bars: report.bars,
        horizon: report.horizon,
        triggers_scanned: report.total_triggers,
        arms_updated,
        policy: policy_label,
        before_plays: before,
        after_plays,
        after_settled,
        arms: arms_out,
    })
}

// --- /api/bandit/reset ----------------------------------
//
// Sprint B：清空 Bandit state，回到 Beta(1,1) 均匀先验
// （用于开发/调试，或市场 regime shift 时手动重置）

fn handle_bandit_reset(ctx: &Ctx) -> Response<std::io::Cursor<Vec<u8>>> {
    let mut guard = match ctx.bandit.lock() {
        Ok(g) => g,
        Err(e) => return json_err(500, format!("bandit lock poisoned: {}", e)),
    };
    *guard = rl::BanditState::new();
    if let Err(e) = rl::save(&ctx.cfg.cache_dir, &guard) {
        return json_err(500, format!("save failed: {}", e));
    }
    json_ok(serde_json::json!({ "reset": true, "version": guard.version }))
}

// --- /api/bandit/decide ---------------------------------
//
// 给定一组候选 arm 名（逗号分隔），按 Thompson Sampling 返回推荐执行者
// 参数：arms=signal.ma.guillotine,playbook.guillotine
//       policy=thompson|ucb1|greedy
//       min_samples=5

#[derive(serde::Serialize)]
struct BanditDecideResp {
    chosen: Option<String>,
    policy: &'static str,
    candidates: Vec<CandidateInfo>,
}

#[derive(serde::Serialize)]
struct CandidateInfo {
    arm: String,
    posterior_mean: f64,
    posterior_variance: f64,
    triggers: u64,
    samples: u64,
}

fn handle_bandit_decide(
    ctx: &Ctx,
    query: &HashMap<String, String>,
) -> Response<std::io::Cursor<Vec<u8>>> {
    let arms_str = match q(query, "arms") {
        Some(v) if !v.is_empty() => v,
        _ => return json_err(400, "参数 arms 必填（逗号分隔的 arm 名）"),
    };
    let candidates: Vec<String> = arms_str
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if candidates.is_empty() {
        return json_err(400, "arms 解析为空");
    }

    let policy = q(query, "policy").unwrap_or("thompson");
    let (policy_enum, policy_label): (rl::SelectionPolicy, &'static str) = match policy {
        "ucb1" => (rl::SelectionPolicy::Ucb1 { c: 2 }, "ucb1"),
        "greedy" => (rl::SelectionPolicy::Greedy, "greedy"),
        _ => (rl::SelectionPolicy::Thompson, "thompson"),
    };
    let min_samples = q(query, "min_samples")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(5);

    let guard = match ctx.bandit.lock() {
        Ok(g) => g,
        Err(e) => return json_err(500, format!("bandit lock poisoned: {}", e)),
    };
    let cand_refs: Vec<&str> = candidates.iter().map(|s| s.as_str()).collect();
    let mut rng = rl::Xoshiro256::from_entropy();
    let chosen = rl::choose(&guard, &cand_refs, policy_enum, &mut rng, min_samples, &[]);

    let cand_info: Vec<CandidateInfo> = candidates
        .iter()
        .map(|name| {
            let (mean, var, trig, samp) = guard
                .arms
                .get(name)
                .map(|a| {
                    (
                        a.posterior_mean(),
                        a.posterior_variance(),
                        a.total_triggers,
                        a.samples(),
                    )
                })
                .unwrap_or((0.5, 1.0 / 12.0, 0, 0)); // Beta(1,1) 的默认
            CandidateInfo {
                arm: name.clone(),
                posterior_mean: mean,
                posterior_variance: var,
                triggers: trig,
                samples: samp,
            }
        })
        .collect();

    json_ok(BanditDecideResp {
        chosen,
        policy: policy_label,
        candidates: cand_info,
    })
}

/// UNIX ms 时间戳
fn unix_now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Sprint B 自动化 3/3：按 debounce 触发后台 live-learn
///
/// 调用点：`handle_decision` / `handle_signals` / `handle_resonance` 的结尾。
/// - 若距离上次 live-learn 超过 [`LIVE_LEARN_DEBOUNCE_MS`]：
///   spawn 后台线程重新 `evaluate` + `merge_report` + 持久化
/// - 否则静默返回（不阻塞请求）
///
/// 语义：后验是**累加**的，当前窗口多被观察几次，高频市场 arm 会占更高权重；
/// 这是 Sprint B 的过渡方案，Sprint C 会切到"每根新 bar 单点 register_trigger"
/// 的真在线学习。
fn maybe_trigger_live_learn(ctx: &Ctx, symbol: &str, tf: Timeframe, limit: usize) {
    let key = format!("{}:{}", symbol, tf.as_str());
    let now = unix_now_ms();

    // 1. 取锁判断是否该跑
    let should_run = {
        let mut guard = match ctx.live_learn_last.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        let last = guard.get(&key).copied().unwrap_or(0);
        if now - last < LIVE_LEARN_DEBOUNCE_MS {
            return;
        }
        // 占位：先写入，避免多个并发请求都触发
        guard.insert(key.clone(), now);
        last
    };
    let _ = should_run;

    // 2. spawn 后台线程
    let bandit = Arc::clone(&ctx.bandit);
    let cache = Arc::clone(&ctx.cache);
    let cache_dir: PathBuf = ctx.cfg.cache_dir.clone().into();
    let sym = symbol.to_string();

    thread::Builder::new()
        .name(format!("live-learn-{}", key))
        .spawn(move || {
            let start = std::time::Instant::now();
            let klines = match cache.get(&sym, tf, limit) {
                Ok(v) => v,
                Err(e) => {
                    log::warn!("live-learn {} {} fetch failed: {}", sym, tf.as_str(), e);
                    return;
                }
            };
            let report = effectiveness::evaluate(
                &klines,
                &sym,
                tf.as_str(),
                effectiveness::DEFAULT_HORIZON,
            );
            let updated = {
                let mut guard = match bandit.lock() {
                    Ok(g) => g,
                    Err(_) => return,
                };
                rl::merge_report(&mut guard, &report, now)
            };

            // 保存（新 scope 避免持锁 IO）
            if let Err(e) = {
                let guard = bandit.lock().expect("bandit lock");
                rl::save(&cache_dir, &guard)
            } {
                log::warn!("live-learn save failed: {}", e);
            } else {
                log::info!(
                    "live-learn {} {} done in {:?}: {} arms merged, {} triggers",
                    sym,
                    tf.as_str(),
                    start.elapsed(),
                    updated,
                    report.total_triggers,
                );
            }
        })
        .ok();
}
