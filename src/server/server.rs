//! HTTP 服务运行循环

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;

use tiny_http::Server;

use crate::config::Config;
use crate::data::{Binance, Bitget, Bybit, KlineCache, Okx, Timeframe};
use crate::engine::{effectiveness, rl};

use super::routes::{dispatch, Ctx};

/// Sprint B 启动 warm-up 默认参数：2000 根 BTCUSDT 4h，horizon=20
const WARMUP_SYMBOL: &str = "BTCUSDT";
const WARMUP_INTERVAL: Timeframe = Timeframe::H4;
const WARMUP_LIMIT: usize = 2000;
const WARMUP_HORIZON: usize = 20;

/// 启动 HTTP 服务（阻塞当前线程）
pub fn run(cfg: Config) -> std::io::Result<()> {
    let bind = cfg.http_bind.clone();
    let server = Server::http(&bind)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

    // 初始化数据源客户端与本地缓存（多交易所）
    let binance = Binance::new(cfg.binance_base.clone());
    let bybit = Bybit::new("https://api.bybit.com");
    let bitget = Bitget::new("https://api.bitget.com");
    let okx = Okx::new("https://www.okx.com");
    let cache =
        Arc::new(KlineCache::new(&cfg.cache_dir, binance, bybit, bitget, okx).with_ttl(60));

    // Sprint B：加载 Bandit 状态
    let initial = rl::load_or_default(&cfg.cache_dir);
    log::info!(
        "Bandit state loaded: {} arms, {} pending, total_plays={}",
        initial.arms.len(),
        initial.pending.len(),
        initial.total_plays,
    );
    let need_warmup = initial.total_plays == 0;
    let bandit = Arc::new(Mutex::new(initial));

    // Sprint B 自动化 2/3：若首次启动（state 为空），后台 warm-up
    if need_warmup {
        let bandit_c = Arc::clone(&bandit);
        let cache_c = Arc::clone(&cache);
        let cache_dir: PathBuf = cfg.cache_dir.clone().into();
        thread::Builder::new()
            .name("bandit-warmup".into())
            .spawn(move || {
                run_warmup(bandit_c, cache_c, cache_dir);
            })
            .map(|_| ())
            .unwrap_or_else(|e| log::warn!("bandit warm-up thread spawn failed: {}", e));
    }

    log::info!("Aura-Trade HTTP 服务启动: http://{}", bind);
    log::info!("健康检查: GET http://{}/api/ping", bind);
    log::info!("K线 API:  GET http://{}/api/klines?symbol=BTCUSDT&interval=4h&limit=500", bind);

    let seed_benchmarks: Arc<Mutex<HashMap<String, Vec<crate::engine::system::BenchmarkSnapshot>>>> =
        Arc::new(Mutex::new(HashMap::new()));

    // M17：后台为所有 hardcoded seeds 跑一次 BTC/ETH/SOL × 1d WF 基准
    // 结果写入 seed_benchmarks（内存），供 /api/system/seeds 注入展示
    {
        let cache_c = Arc::clone(&cache);
        let bench_c = Arc::clone(&seed_benchmarks);
        thread::Builder::new()
            .name("seed-benchmark".into())
            .spawn(move || {
                run_seed_benchmark(cache_c, bench_c);
            })
            .map(|_| ())
            .unwrap_or_else(|e| log::warn!("seed-benchmark thread spawn failed: {}", e));
    }

    let ctx = Ctx {
        cfg: Arc::new(cfg),
        cache,
        bandit,
        live_learn_last: Arc::new(Mutex::new(HashMap::new())),
        seed_benchmarks,
        symbols_cache: Arc::new(Mutex::new(None)),
    };

    // Phase 1：单线程处理（tiny_http 默认 blocking accept）
    // 后续 Phase 可升级为 thread pool（rayon / 手写线程池）
    for req in server.incoming_requests() {
        dispatch(&ctx, req);
    }
    Ok(())
}

/// M17：后台跑所有 hardcoded seed 的基准 WF，结果填入共享 map
fn run_seed_benchmark(
    cache: Arc<KlineCache>,
    out: Arc<Mutex<HashMap<String, Vec<crate::engine::system::BenchmarkSnapshot>>>>,
) {
    use crate::engine::system::{all_seeds, run_benchmark_with, BenchmarkSnapshot};

    let t0 = std::time::Instant::now();
    log::info!("Seed benchmark 启动：跑 hardcoded seeds × BTC/ETH/SOL × 1d × 4 folds");

    let seeds = all_seeds();
    let symbols: Vec<String> = ["BTCUSDT", "ETHUSDT", "SOLUSDT"].iter().map(|s| s.to_string()).collect();
    let intervals = vec!["1d".to_string()];

    let tf = crate::data::Timeframe::D1;
    let report = run_benchmark_with(
        &seeds,
        &symbols,
        &intervals,
        4,
        |sym, _tf| cache.get(&sym.to_uppercase(), tf, 2000).ok(),
    );

    // 按 system_id 分组 -> Vec<BenchmarkSnapshot>
    let mut grouped: HashMap<String, Vec<BenchmarkSnapshot>> = HashMap::new();
    for cell in &report.cells {
        if cell.error.is_some() {
            continue;
        }
        grouped.entry(cell.system_id.clone()).or_default().push(BenchmarkSnapshot {
            symbol: cell.symbol.clone(),
            interval: cell.interval.clone(),
            wf_consistency: cell.wf_consistency,
            wf_avg_sharpe: cell.wf_avg_sharpe,
            wf_avg_return_pct: cell.wf_avg_return_pct,
            total_trades: cell.total_trades,
        });
    }

    if let Ok(mut guard) = out.lock() {
        *guard = grouped;
    }
    log::info!(
        "Seed benchmark 完成：{} seeds × {} cells 耗时 {:?}",
        seeds.len(),
        report.cells.len(),
        t0.elapsed()
    );
}

/// 后台 warm-up：下载 BTCUSDT 4h × 2000 根，评估后合并到 Bandit state
fn run_warmup(
    bandit: Arc<Mutex<rl::BanditState>>,
    cache: Arc<KlineCache>,
    cache_dir: PathBuf,
) {
    log::info!(
        "Bandit warm-up starting: {} {} × {} bars, horizon={}",
        WARMUP_SYMBOL,
        WARMUP_INTERVAL.as_str(),
        WARMUP_LIMIT,
        WARMUP_HORIZON,
    );
    let start = std::time::Instant::now();

    let klines = match cache.get(WARMUP_SYMBOL, WARMUP_INTERVAL, WARMUP_LIMIT) {
        Ok(v) => v,
        Err(e) => {
            log::warn!("Bandit warm-up aborted: fetch klines failed: {}", e);
            return;
        }
    };
    let report = effectiveness::evaluate(
        &klines,
        WARMUP_SYMBOL,
        WARMUP_INTERVAL.as_str(),
        WARMUP_HORIZON,
    );

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let updated = {
        let mut guard = match bandit.lock() {
            Ok(g) => g,
            Err(e) => {
                log::warn!("Bandit warm-up: lock poisoned: {}", e);
                return;
            }
        };
        if guard.total_plays > 0 {
            log::info!("Bandit warm-up skipped: state already populated by other request");
            return;
        }
        rl::merge_report(&mut guard, &report, now_ms)
    };

    if let Err(e) = {
        let guard = bandit.lock().expect("bandit lock");
        rl::save(&cache_dir, &guard)
    } {
        log::warn!("Bandit warm-up: save failed: {}", e);
    } else {
        log::info!(
            "Bandit warm-up done in {:?}: {} arms, {} triggers",
            start.elapsed(),
            updated,
            report.total_triggers
        );
    }
}
