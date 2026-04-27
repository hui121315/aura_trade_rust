//! 体系实验室 HTTP 端点（M4）
//!
//! 为前端体系实验室 UI 提供 3 个 REST 端点：
//!
//! | Method | Path                     | 用途                                 |
//! |--------|--------------------------|--------------------------------------|
//! | GET    | `/api/system/components` | 列出全部 32 个组件（按维度分组）     |
//! | GET    | `/api/system/seeds`      | 列出全部 8 个种子体系                |
//! | POST   | `/api/system/run`        | 提交 `SystemDefinition` JSON 跑回测  |
//!
//! 所有响应使用项目统一的 `{ok, data, error}` envelope。

use std::collections::BTreeMap;
#[allow(unused_imports)]
use std::io::Read; // trait needed for `as_reader().read_to_string()`

use tiny_http::{Request, Response};

use crate::data::Timeframe;
use crate::engine::system::{
    self, add_promoted, all_components, all_seeds, discover, load_promoted, remove_promoted,
    run_benchmark_with, run_walkforward, BenchmarkSnapshot, Component, ComponentDimension,
    DiscoveryConfig, SystemDefinition, WalkForwardConfig,
};

use super::response::{json_err, json_ok};
use super::routes::Ctx;

// ============================================================
// GET /api/system/components
// ============================================================

#[derive(serde::Serialize)]
struct ComponentsResp {
    total: usize,
    by_dimension: BTreeMap<String, Vec<ComponentOut>>,
}

#[derive(serde::Serialize)]
struct ComponentOut {
    id: &'static str,
    label: &'static str,
    book_source: &'static str,
    dimension: String,
    direction_bias: i8,
    historical_alpha_pct: Option<f64>,
    historical_winrate: Option<f64>,
}

pub fn handle_list_components() -> Response<std::io::Cursor<Vec<u8>>> {
    let mut by_dim: BTreeMap<String, Vec<ComponentOut>> = BTreeMap::new();
    for c in all_components() {
        by_dim
            .entry(format!("{:?}", c.dimension))
            .or_default()
            .push(component_to_out(c));
    }
    json_ok(ComponentsResp {
        total: all_components().len(),
        by_dimension: by_dim,
    })
}

fn component_to_out(c: &Component) -> ComponentOut {
    ComponentOut {
        id: c.id,
        label: c.label,
        book_source: c.book_source,
        dimension: format!("{:?}", c.dimension),
        direction_bias: c.direction_bias,
        historical_alpha_pct: c.historical_alpha_pct,
        historical_winrate: c.historical_winrate,
    }
}

// 临时抑制未用 ComponentDimension import 的警告（为未来按维度筛选预留）
#[allow(dead_code)]
fn _keep_component_dimension(_: ComponentDimension) {}

// ============================================================
// GET /api/system/seeds
// ============================================================

pub fn handle_list_seeds(ctx: &Ctx) -> Response<std::io::Cursor<Vec<u8>>> {
    let mut seeds: Vec<SystemDefinition> = all_seeds();
    let cache_dir = std::path::Path::new(&ctx.cfg.cache_dir);
    let mut promoted = load_promoted(cache_dir);
    let promoted_count = promoted.len();

    // M17：若 hardcoded seed 自己的 meta.last_benchmark 为空，用后台算好的填充
    if let Ok(bench_map) = ctx.seed_benchmarks.lock() {
        for s in seeds.iter_mut() {
            if s.meta.last_benchmark.is_empty() {
                if let Some(snapshots) = bench_map.get(&s.id) {
                    s.meta.last_benchmark = snapshots.clone();
                }
            }
        }
    }

    seeds.append(&mut promoted);
    #[derive(serde::Serialize)]
    struct SeedsResp {
        total: usize,
        seed_count: usize,
        promoted_count: usize,
        seeds: Vec<SystemDefinition>,
    }
    json_ok(SeedsResp {
        total: seeds.len(),
        seed_count: seeds.len() - promoted_count,
        promoted_count,
        seeds,
    })
}

// ============================================================
// POST /api/system/run
// ============================================================

#[derive(serde::Deserialize)]
struct RunRequest {
    /// 体系定义（可以是种子体系也可以是用户自定义）
    definition: SystemDefinition,
    /// 市场（默认 BTCUSDT）
    #[serde(default = "default_symbol")]
    symbol: String,
    /// 周期（默认 1d）
    #[serde(default = "default_interval")]
    interval: String,
    /// 回测根数（默认 1000）
    #[serde(default = "default_limit")]
    limit: usize,
}

fn default_symbol() -> String { "BTCUSDT".into() }
fn default_interval() -> String { "1d".into() }
fn default_limit() -> usize { 1000 }

pub fn handle_run(ctx: &Ctx, mut req: Request) -> std::io::Result<()> {
    let mut body = String::new();
    if let Err(e) = req.as_reader().read_to_string(&mut body) {
        return req.respond(json_err(400, format!("读取请求体失败: {}", e)));
    }
    let parsed: RunRequest = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(e) => {
            return req.respond(json_err(400, format!("JSON 解析失败: {}", e)));
        }
    };

    // 参数校验与 clamp
    let symbol = parsed.symbol.to_uppercase();
    let tf = match Timeframe::parse(&parsed.interval) {
        Some(tf) => tf,
        None => {
            return req.respond(json_err(400, format!("非法 interval: {}", parsed.interval)));
        }
    };
    let limit = parsed.limit.clamp(100, 5000);

    // 拉 K 线（走本地缓存）
    let klines = match ctx.cache.get(&symbol, tf, limit) {
        Ok(v) => v,
        Err(e) => return req.respond(json_err(502, format!("拉取 K线失败: {}", e))),
    };

    // 跑回测
    let result = match system::run(&parsed.definition, &klines, &symbol, tf.as_str()) {
        Ok(r) => r,
        Err(e) => return req.respond(json_err(400, format!("体系校验或执行失败: {}", e))),
    };

    req.respond(json_ok(result))
}

// ============================================================
// POST /api/system/walkforward
// ============================================================

#[derive(serde::Deserialize)]
struct WalkForwardRequest {
    definition: SystemDefinition,
    #[serde(default = "default_symbol")]
    symbol: String,
    #[serde(default = "default_interval")]
    interval: String,
    #[serde(default = "default_wf_limit")]
    limit: usize,
    #[serde(default = "default_folds")]
    folds: usize,
    #[serde(default)]
    prewarm_bars: usize,
}

fn default_wf_limit() -> usize { 2000 }
fn default_folds() -> usize { 4 }

pub fn handle_walkforward(ctx: &Ctx, mut req: Request) -> std::io::Result<()> {
    let mut body = String::new();
    if let Err(e) = req.as_reader().read_to_string(&mut body) {
        return req.respond(json_err(400, format!("读取请求体失败: {}", e)));
    }
    let parsed: WalkForwardRequest = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(e) => return req.respond(json_err(400, format!("JSON 解析失败: {}", e))),
    };

    let symbol = parsed.symbol.to_uppercase();
    let tf = match Timeframe::parse(&parsed.interval) {
        Some(tf) => tf,
        None => return req.respond(json_err(400, format!("非法 interval: {}", parsed.interval))),
    };
    let limit = parsed.limit.clamp(200, 5000);

    let klines = match ctx.cache.get(&symbol, tf, limit) {
        Ok(v) => v,
        Err(e) => return req.respond(json_err(502, format!("拉取 K线失败: {}", e))),
    };

    let cfg = WalkForwardConfig {
        folds: parsed.folds,
        prewarm_bars: parsed.prewarm_bars,
    };
    let report = match run_walkforward(&parsed.definition, &klines, &symbol, tf.as_str(), &cfg) {
        Ok(r) => r,
        Err(e) => return req.respond(json_err(400, format!("Walk-forward 执行失败: {}", e))),
    };

    req.respond(json_ok(report))
}

// ============================================================
// POST /api/system/discover
// ============================================================

#[derive(serde::Deserialize)]
struct DiscoverRequest {
    #[serde(default = "default_symbol")]
    symbol: String,
    #[serde(default = "default_interval")]
    interval: String,
    #[serde(default = "default_wf_limit")]
    limit: usize,
    /// 交叉验证的额外 symbol 列表（可选）
    #[serde(default)]
    cross_symbols: Vec<String>,
    /// M11：交叉验证的额外周期（可选）。会与主 symbol + cross_symbols 做笛卡尔积
    #[serde(default)]
    cross_intervals: Vec<String>,
    #[serde(flatten)]
    config: DiscoveryConfig,
}

pub fn handle_discover(ctx: &Ctx, mut req: Request) -> std::io::Result<()> {
    let mut body = String::new();
    if let Err(e) = req.as_reader().read_to_string(&mut body) {
        return req.respond(json_err(400, format!("读取请求体失败: {}", e)));
    }
    let parsed: DiscoverRequest = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(e) => return req.respond(json_err(400, format!("JSON 解析失败: {}", e))),
    };
    let symbol = parsed.symbol.to_uppercase();
    let tf = match Timeframe::parse(&parsed.interval) {
        Some(tf) => tf,
        None => return req.respond(json_err(400, format!("非法 interval: {}", parsed.interval))),
    };
    let limit = parsed.limit.clamp(400, 5000);

    let klines = match ctx.cache.get(&symbol, tf, limit) {
        Ok(v) => v,
        Err(e) => return req.respond(json_err(502, format!("拉取 K线失败: {}", e))),
    };

    // M11：构造验证点的笛卡尔积
    //
    // 规则：主 symbol + cross_symbols 的所有 symbol × cross_intervals 的所有周期，
    // 再加上 cross_symbols 在主 interval 上（保持 M6 的默认行为）。
    // 去除 (主 symbol, 主 interval) 这个组合（主体已经跑过）。
    let all_cross_symbols: Vec<String> = std::iter::once(symbol.clone())
        .chain(parsed.cross_symbols.iter().map(|s| s.to_uppercase()))
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    let all_cross_intervals: Vec<String> =
        std::iter::once(parsed.interval.to_lowercase())
            .chain(parsed.cross_intervals.iter().map(|s| s.to_lowercase()))
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();

    let mut cross_owned: Vec<(String, String, Vec<crate::data::Kline>)> = Vec::new();
    for csym in &all_cross_symbols {
        for citv in &all_cross_intervals {
            // 主 symbol × 主 interval 已经在外面单独跑，跳过
            if csym == &symbol && citv == parsed.interval.to_lowercase().as_str() {
                continue;
            }
            let Some(ctf) = Timeframe::parse(citv) else {
                return req.respond(json_err(400, format!("非法 cross interval: {}", citv)));
            };
            match ctx.cache.get(csym, ctf, limit) {
                Ok(kl) => cross_owned.push((csym.clone(), citv.clone(), kl)),
                Err(e) => {
                    return req.respond(json_err(
                        502,
                        format!("拉取 {} {} K线失败: {}", csym, citv, e),
                    ))
                }
            }
        }
    }
    let cross_refs: Vec<(&str, &str, &[crate::data::Kline])> = cross_owned
        .iter()
        .map(|(s, tf, k)| (s.as_str(), tf.as_str(), k.as_slice()))
        .collect();

    let report = match discover(&klines, &symbol, tf.as_str(), &parsed.config, &cross_refs) {
        Ok(r) => r,
        Err(e) => return req.respond(json_err(400, format!("Discovery 执行失败: {}", e))),
    };
    req.respond(json_ok(report))
}

// ============================================================
// POST /api/system/promote  —  入库一个体系
// POST /api/system/demote   —  按 id 移除一个已入库体系
// ============================================================

#[derive(serde::Deserialize)]
struct PromoteRequest {
    definition: SystemDefinition,
}

pub fn handle_promote(ctx: &Ctx, mut req: Request) -> std::io::Result<()> {
    let mut body = String::new();
    if let Err(e) = req.as_reader().read_to_string(&mut body) {
        return req.respond(json_err(400, format!("读取请求体失败: {}", e)));
    }
    let parsed: PromoteRequest = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(e) => return req.respond(json_err(400, format!("JSON 解析失败: {}", e))),
    };
    let cache_dir = std::path::Path::new(&ctx.cfg.cache_dir);

    // Step 1: 入库得到规范化的 id + origin
    let mut saved = match add_promoted(cache_dir, parsed.definition) {
        Ok(s) => s,
        Err(e) => return req.respond(json_err(400, format!("入库失败: {}", e))),
    };

    // Step 2: 自动跑 BTC/ETH/SOL × 1d × 4 折 WF benchmark（并行，~30ms）
    let benchmark_symbols = ["BTCUSDT", "ETHUSDT", "SOLUSDT"];
    let benchmark_interval = "1d";
    let cache = ctx.cache.clone();
    let tf = match crate::data::Timeframe::parse(benchmark_interval) {
        Some(v) => v,
        None => {
            // 极端情况；返回不带 benchmark 的结果
            return req.respond(json_ok(PromoteResp { definition: saved }));
        }
    };
    let report = run_benchmark_with(
        std::slice::from_ref(&saved),
        &benchmark_symbols.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
        &[benchmark_interval.to_string()],
        4,
        |sym, _tf| cache.get(&sym.to_uppercase(), tf, 2000).ok(),
    );

    // Step 3: 把 cells 写回 meta
    let snapshots: Vec<BenchmarkSnapshot> = report
        .cells
        .iter()
        .filter(|c| c.error.is_none())
        .map(|c| BenchmarkSnapshot {
            symbol: c.symbol.clone(),
            interval: c.interval.clone(),
            wf_consistency: c.wf_consistency,
            wf_avg_sharpe: c.wf_avg_sharpe,
            wf_avg_return_pct: c.wf_avg_return_pct,
            total_trades: c.total_trades,
        })
        .collect();
    saved.meta.last_benchmark = snapshots;
    saved.meta.last_benchmark_at_ms = Some(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0),
    );

    // Step 4: 再次保存（覆盖同 id）
    let final_def = match add_promoted(cache_dir, saved) {
        Ok(s) => s,
        Err(e) => {
            log::warn!("保存带 benchmark 的体系失败: {}", e);
            return req.respond(json_err(500, format!("保存失败: {}", e)));
        }
    };

    req.respond(json_ok(PromoteResp { definition: final_def }))
}

#[derive(serde::Serialize)]
struct PromoteResp {
    definition: SystemDefinition,
}

#[derive(serde::Deserialize)]
struct DemoteRequest {
    id: String,
}

// ============================================================
// POST /api/system/benchmark
// ============================================================

#[derive(serde::Deserialize)]
struct BenchmarkRequest {
    /// 指定体系 id 子集；空或缺省 = 所有 seeds + promoted
    #[serde(default)]
    system_ids: Vec<String>,
    #[serde(default = "default_benchmark_symbols")]
    symbols: Vec<String>,
    #[serde(default = "default_benchmark_intervals")]
    intervals: Vec<String>,
    #[serde(default = "default_benchmark_limit")]
    limit: usize,
    #[serde(default = "default_folds")]
    folds: usize,
}

fn default_benchmark_symbols() -> Vec<String> {
    vec!["BTCUSDT", "ETHUSDT", "SOLUSDT"].into_iter().map(String::from).collect()
}
fn default_benchmark_intervals() -> Vec<String> {
    vec!["1d".to_string()]
}
fn default_benchmark_limit() -> usize { 2000 }

pub fn handle_benchmark(ctx: &Ctx, mut req: Request) -> std::io::Result<()> {
    let mut body = String::new();
    if let Err(e) = req.as_reader().read_to_string(&mut body) {
        return req.respond(json_err(400, format!("读取请求体失败: {}", e)));
    }
    let parsed: BenchmarkRequest = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(e) => return req.respond(json_err(400, format!("JSON 解析失败: {}", e))),
    };

    // 合并 hardcoded + promoted
    let cache_dir = std::path::Path::new(&ctx.cfg.cache_dir);
    let mut all: Vec<SystemDefinition> = all_seeds();
    all.extend(load_promoted(cache_dir));

    let systems: Vec<SystemDefinition> = if parsed.system_ids.is_empty() {
        all
    } else {
        let wanted: std::collections::HashSet<&str> =
            parsed.system_ids.iter().map(|s| s.as_str()).collect();
        all.into_iter().filter(|s| wanted.contains(s.id.as_str())).collect()
    };
    if systems.is_empty() {
        return req.respond(json_err(400, "没有匹配的体系"));
    }

    let limit = parsed.limit.clamp(400, 5000);

    // 预拉所有 (symbol, interval) klines（KlineCache 内部已有 60s TTL）
    let cache = ctx.cache.clone();
    let report = run_benchmark_with(
        &systems,
        &parsed.symbols,
        &parsed.intervals,
        parsed.folds,
        |sym, tf| {
            let Some(tf_parsed) = Timeframe::parse(tf) else {
                log::warn!("非法 interval: {}", tf);
                return None;
            };
            match cache.get(&sym.to_uppercase(), tf_parsed, limit) {
                Ok(kl) => Some(kl),
                Err(e) => {
                    log::warn!("拉取 {} {} 失败: {}", sym, tf, e);
                    None
                }
            }
        },
    );
    req.respond(json_ok(report))
}

// ============================================================
// POST /api/system/live_scan  —  轻量扫描：返回最近 N bar 的组件触发
// ============================================================

#[derive(serde::Deserialize)]
struct LiveScanRequest {
    definition: SystemDefinition,
    #[serde(default = "default_symbol")]
    symbol: String,
    #[serde(default = "default_interval")]
    interval: String,
    /// 拉多少根 K 线（默认 300，足够让 MA250 稳定）
    #[serde(default = "default_live_limit")]
    limit: usize,
    /// 返回最近 N bar 的详细触发（默认 5）
    #[serde(default = "default_tail_bars")]
    tail_bars: usize,
}

fn default_live_limit() -> usize { 300 }
fn default_tail_bars() -> usize { 100 }

#[derive(serde::Serialize)]
struct LiveScanBarResult {
    bar_index: usize,
    open_time: i64,
    close_time: i64,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    /// 当前 bar 各组件触发情况（有触发的才会出现）
    triggers: Vec<LiveScanTrigger>,
    /// 聚合结果：是否产生交易信号
    combined_direction: i8,
    combined_fired: bool,
}

#[derive(serde::Serialize)]
struct LiveScanTrigger {
    component_id: String,
    component_label: String,
    direction: i8,
    confidence: f64,
    reason: String,
}

#[derive(serde::Serialize)]
struct LiveScanResp {
    symbol: String,
    interval: String,
    total_bars: usize,
    latest_close: f64,
    latest_close_time: i64,
    /// 最近 N bar 的每 bar 详细触发，最新 bar 在末尾
    bars: Vec<LiveScanBarResult>,
    /// 整段统计（最近 limit bar）：按组件 id → 触发总数
    total_triggers_by_component: std::collections::BTreeMap<String, usize>,
}

pub fn handle_live_scan(ctx: &Ctx, mut req: Request) -> std::io::Result<()> {
    let mut body = String::new();
    if let Err(e) = req.as_reader().read_to_string(&mut body) {
        return req.respond(json_err(400, format!("读取请求体失败: {}", e)));
    }
    let parsed: LiveScanRequest = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(e) => return req.respond(json_err(400, format!("JSON 解析失败: {}", e))),
    };

    let symbol = parsed.symbol.to_uppercase();
    let tf = match Timeframe::parse(&parsed.interval) {
        Some(tf) => tf,
        None => return req.respond(json_err(400, format!("非法 interval: {}", parsed.interval))),
    };
    let limit = parsed.limit.clamp(100, 2000);
    let tail = parsed.tail_bars.clamp(1, 1000);

    let klines = match ctx.cache.get(&symbol, tf, limit) {
        Ok(v) => v,
        Err(e) => return req.respond(json_err(502, format!("拉取 K线失败: {}", e))),
    };

    let def = &parsed.definition;

    // 扫描一次（整段）
    let scan = system::scan::scan_all_triggers(&klines);

    // 校验体系组件是否都已知
    for cid in &def.components {
        if system::find_component(cid).is_none() {
            return req.respond(json_err(400, format!("未知组件 id: {}", cid)));
        }
    }

    let label_of = |cid: &str| -> String {
        system::find_component(cid)
            .map(|c| c.label.to_string())
            .unwrap_or_else(|| cid.to_string())
    };

    let total_bars = klines.len();
    let tail_start = total_bars.saturating_sub(tail);

    let mut bars: Vec<LiveScanBarResult> = Vec::with_capacity(tail);
    for i in tail_start..total_bars {
        let k = &klines[i];
        // 当前 bar 上每个组件的触发（None = 未触发）
        let per_component: Vec<(String, Option<&system::TriggerEvent>)> = def
            .components
            .iter()
            .map(|cid| (cid.clone(), scan.get_trigger(cid.as_str(), i)))
            .collect();

        // 仅触发到的组件进入明细
        let triggers: Vec<LiveScanTrigger> = per_component
            .iter()
            .filter_map(|(cid, ev)| {
                ev.map(|e| LiveScanTrigger {
                    component_id: cid.clone(),
                    component_label: label_of(cid),
                    direction: e.direction,
                    confidence: e.confidence,
                    reason: e.reason.clone(),
                })
            })
            .collect();

        // 聚合
        let ctx = system::combine::CombineCtx {
            scan: &scan,
            current_bar: i,
            components: &def.components,
        };
        let combined = system::combine::evaluate_combine(
            &per_component,
            &def.combine,
            &def.weights,
            Some(&ctx),
        );
        let (combined_direction, combined_fired) = match combined {
            Some(sig) => (sig.direction, sig.direction != 0),
            None => (0, false),
        };

        bars.push(LiveScanBarResult {
            bar_index: i,
            open_time: k.open_time,
            close_time: k.close_time,
            open: k.open,
            high: k.high,
            low: k.low,
            close: k.close,
            triggers,
            combined_direction,
            combined_fired,
        });
    }

    // 整段按组件 ID 汇总触发数
    let mut total_by: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for cid in &def.components {
        let n = scan
            .triggers
            .get(cid.as_str())
            .map(|v| v.len())
            .unwrap_or(0);
        total_by.insert(cid.clone(), n);
    }

    let latest = klines.last();
    let resp = LiveScanResp {
        symbol: symbol.clone(),
        interval: tf.as_str().to_string(),
        total_bars,
        latest_close: latest.map(|k| k.close).unwrap_or(0.0),
        latest_close_time: latest.map(|k| k.close_time).unwrap_or(0),
        bars,
        total_triggers_by_component: total_by,
    };
    req.respond(json_ok(resp))
}

pub fn handle_demote(ctx: &Ctx, mut req: Request) -> std::io::Result<()> {
    let mut body = String::new();
    if let Err(e) = req.as_reader().read_to_string(&mut body) {
        return req.respond(json_err(400, format!("读取请求体失败: {}", e)));
    }
    let parsed: DemoteRequest = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(e) => return req.respond(json_err(400, format!("JSON 解析失败: {}", e))),
    };
    let cache_dir = std::path::Path::new(&ctx.cfg.cache_dir);
    match remove_promoted(cache_dir, &parsed.id) {
        Ok(removed) => {
            #[derive(serde::Serialize)]
            struct Resp { removed: bool }
            req.respond(json_ok(Resp { removed }))
        }
        Err(e) => req.respond(json_err(400, format!("移除失败: {}", e))),
    }
}

