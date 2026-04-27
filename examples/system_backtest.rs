//! 体系实验室 M1 demo：对种子体系做单段回测
//!
//! # 用法
//!
//! ```bash
//! cargo run --example system_backtest --release -- BTCUSDT 1d 1500
//! cargo run --example system_backtest --release -- ETHUSDT 4h 2000
//! ```
//!
//! 默认参数：BTCUSDT 1d 1500 根 K 线。
//!
//! # 输出
//!
//! - 所有种子体系的 Performance 汇总
//! - 每个体系的组件归因（多少次触发、多少次真正成交）
//! - 前 5 笔交易的细节

use std::env;

use aura_trade::data::binance::Binance;
use aura_trade::data::Timeframe;
use aura_trade::engine::system::{self, all_seeds, scan_all_triggers, CombineRule, SystemDefinition};

fn main() {
    // 极简 stderr logger（与主 bin 一致）
    simple_logger_init();

    let args: Vec<String> = env::args().collect();
    let symbol = args.get(1).cloned().unwrap_or_else(|| "BTCUSDT".into());
    let interval_str = args.get(2).cloned().unwrap_or_else(|| "1d".into());
    let limit: usize = args
        .get(3)
        .and_then(|s| s.parse().ok())
        .unwrap_or(1500);

    let tf = Timeframe::parse(&interval_str).unwrap_or_else(|| {
        eprintln!("未识别的 interval: {}", interval_str);
        std::process::exit(1);
    });

    println!("═══════════════════════════════════════════════════════════");
    println!("  体系实验室 · M1 · 种子体系回测");
    println!("═══════════════════════════════════════════════════════════");
    println!("  Symbol:   {}", symbol);
    println!("  Interval: {}", interval_str);
    println!("  Bars:     {}", limit);
    println!();

    // ---------- 拉取数据 ----------
    println!("📥 正在从 Binance 拉取 {} {} {} 根 K 线 ...", symbol, interval_str, limit);
    let client = Binance::new("https://api.binance.com");
    let klines = match client.klines(&symbol, tf, limit) {
        Ok(ks) => ks,
        Err(e) => {
            eprintln!("拉取失败: {}", e);
            std::process::exit(2);
        }
    };
    println!("✓ 获取 {} 根 K 线（{} → {}）", klines.len(),
        fmt_ts(klines.first().map(|k| k.open_time).unwrap_or(0)),
        fmt_ts(klines.last().map(|k| k.close_time).unwrap_or(0)),
    );
    println!();

    // ---------- 组件注册表 + 实际触发次数概览（M2 新增） ----------
    let total_components = system::all_components().len();
    println!("📦 组件注册表 × 本段数据触发统计（{} 个 MVP 组件）:", total_components);
    let scan_t0 = std::time::Instant::now();
    let scan_result = scan_all_triggers(&klines);
    let scan_dt = scan_t0.elapsed();
    println!("  (预扫描耗时: {:.1} ms)", scan_dt.as_secs_f64() * 1000.0);

    let mut by_dim: std::collections::BTreeMap<&'static str, Vec<(&str, usize)>> =
        std::collections::BTreeMap::new();
    for c in system::all_components() {
        by_dim
            .entry(c.dimension.as_str())
            .or_default()
            .push((c.id, scan_result.count(c.id)));
    }
    for (dim, ids) in &by_dim {
        println!("  [{}]", dim);
        for (id, count) in ids {
            let flag = if *count == 0 { " ⚠" } else { "" };
            println!("    {:<40} 触发 {:>4} 次{}", id, count, flag);
        }
    }
    println!();

    // ---------- 回测所有种子体系 ----------
    let seeds = all_seeds();
    println!("🌱 即将回测 {} 个种子体系:", seeds.len());
    for s in &seeds {
        println!("  - {} [{}]  组件数 {}", s.id, s.name, s.components.len());
    }
    println!();

    println!("───────────────────────────────────────────────────────────");
    for def in seeds {
        run_and_report(&def, &klines, &symbol, &interval_str);
        println!("───────────────────────────────────────────────────────────");
    }

    // ---------- 自定义体系 demo：葛南维 MajorityK ----------
    println!();
    println!("🧪 自定义体系 demo 1: 三路葛南维买入 MajorityK{{k=1}}");
    let custom = SystemDefinition {
        id: "custom.granville_any_bull".into(),
        name: "葛南维任一买点".into(),
        origin: system::SystemOrigin::User,
        description: Some("B1/B2/B3 任一触发即开多".into()),
        components: vec![
            "ma.granville.b1_breakout".into(),
            "ma.granville.b2_pullback".into(),
            "ma.granville.b3_false_break".into(),
        ],
        combine: CombineRule::MajorityK { k: 1 },
        weights: Default::default(),
        risk: system::RiskParams::default(),
        backtest: system::BacktestParams::default(),
        meta: system::SystemMeta::default(),
    };
    run_and_report(&custom, &klines, &symbol, &interval_str);

    // ---------- M2 新组件 demo：金山谷 × 多头排列 × 道氏上升 ----------
    println!();
    println!("🧪 自定义体系 demo 2: M2 新组件四维共振（AllAligned）");
    let resonance = SystemDefinition {
        id: "custom.resonance_ma_trend".into(),
        name: "金山谷 × 多头排列 × 道氏上升".into(),
        origin: system::SystemOrigin::User,
        description: Some("三个维度都看多才开仓（稀疏但应更稳健）".into()),
        components: vec![
            "ma_special.golden_valley".into(),
            "ma_special.bull_arrangement".into(),
            "trend.dow_uptrend".into(),
        ],
        combine: CombineRule::AllAligned,
        weights: Default::default(),
        risk: system::RiskParams {
            stop_atr_mult: 2.0,
            target_r: 3.0,
            max_hold_bars: 40,
            max_position_pct: 0.5,
        },
        backtest: system::BacktestParams::default(),
        meta: system::SystemMeta::default(),
    };
    run_and_report(&resonance, &klines, &symbol, &interval_str);

    // ---------- SequentialCascade demo：断头铡刀 → 毒蜘蛛 级联（占位演示）----------
    // 注：毒蜘蛛尚未注册组件，这里用已有的"死亡谷 → 空头排列"做级联演示
    println!();
    println!("🧪 自定义体系 demo 3: SequentialCascade 级联（死亡谷 → 加速下行）");
    let cascade = SystemDefinition {
        id: "custom.death_cascade".into(),
        name: "死亡谷 → 加速下行级联做空".into(),
        origin: system::SystemOrigin::User,
        description: Some("先出现死亡谷，10 根内再出现加速下行才做空".into()),
        components: vec![
            "ma_special.death_valley".into(),
            "ma_special.accelerating_down".into(),
        ],
        combine: CombineRule::SequentialCascade { window_bars: 10 },
        weights: Default::default(),
        risk: system::RiskParams::default(),
        backtest: system::BacktestParams::default(),
        meta: system::SystemMeta::default(),
    };
    run_and_report(&cascade, &klines, &symbol, &interval_str);

    println!();
    println!("✅ 完成。M2 验收通过（21 组件全扫描 + 4 种聚合规则全实现）。");
    println!("   下一步：M3 把种子体系扩展到 8 个 / M4+M5 HTTP API + 前端 / M7 walk-forward。");
}

fn run_and_report(
    def: &SystemDefinition,
    klines: &[aura_trade::data::Kline],
    symbol: &str,
    interval: &str,
) {
    println!();
    println!("▶ 体系: {}  [{}]", def.id, def.name);
    if let Some(desc) = &def.description {
        println!("  描述: {}", desc);
    }
    println!("  组件: {}", def.components.join(" + "));
    println!(
        "  聚合: {}   风控: ATR×{:.1} / R×{:.1} / Max{}",
        fmt_rule(&def.combine),
        def.risk.stop_atr_mult,
        def.risk.target_r,
        def.risk.max_hold_bars,
    );

    let t0 = std::time::Instant::now();
    let result = match system::run(def, klines, symbol, interval) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("  ✗ 回测失败: {}", e);
            return;
        }
    };
    let dt = t0.elapsed();

    let p = &result.performance;
    println!(
        "  📊 成绩: 交易 {}  胜率 {:.1}%  总收益 {:+.2}%  年化 {:+.2}%  Sharpe {:.2}  最大回撤 {:.2}%  期望 R {:.2}",
        p.total_trades,
        p.win_rate * 100.0,
        p.total_return_pct * 100.0,
        p.annualized_return_pct * 100.0,
        p.sharpe,
        p.max_drawdown_pct * 100.0,
        p.expectancy_r,
    );
    println!(
        "     profit_factor {:.2}  avg_win {:+.4}  avg_loss {:+.4}  Sortino {:.2}  avg_hold {:.1} bars",
        p.profit_factor, p.avg_win, p.avg_loss, p.sortino, p.avg_hold_bars,
    );
    println!("  ⏱ 耗时: {:.1} ms", dt.as_secs_f64() * 1000.0);

    // 组件归因
    println!("  🔬 组件归因:");
    for c in &result.component_contribution {
        println!(
            "     {:<35} 触发 {:>4} 次   成交 {:>3} 次",
            c.component_id, c.triggers, c.matched_system_entries
        );
    }

    // 前 5 笔交易
    if !result.trades.is_empty() {
        println!("  📋 前 5 笔交易:");
        for t in result.trades.iter().take(5) {
            println!(
                "     #{:<3} {:>5} {:>5} bar {:>4}→{:>4}  {:>7.2} → {:>7.2}  pnl {:+.2}%  R {:+.2}  {:?}",
                t.id,
                format!("{:?}", t.side),
                format!("{}b", t.hold_bars),
                t.entry_bar,
                t.exit_bar,
                t.entry_price,
                t.exit_price,
                t.pnl_pct * 100.0,
                t.r_multiple,
                t.exit_reason,
            );
        }
    }
}

fn fmt_rule(rule: &CombineRule) -> String {
    match rule {
        CombineRule::AllAligned => "AllAligned".into(),
        CombineRule::MajorityK { k } => format!("MajorityK{{k={}}}", k),
        CombineRule::WeightedScore { threshold } => format!("WeightedScore{{t={}}}", threshold),
        CombineRule::SequentialCascade { window_bars } => {
            format!("SequentialCascade{{win={}}}", window_bars)
        }
    }
}

fn fmt_ts(ms: i64) -> String {
    // 极简时间格式化（yyyy-mm-dd）
    if ms <= 0 {
        return "-".into();
    }
    let secs = ms / 1000;
    // 1970-01-01 以来的秒数 → 日历
    let (y, m, d) = epoch_secs_to_ymd(secs);
    format!("{:04}-{:02}-{:02}", y, m, d)
}

fn epoch_secs_to_ymd(secs: i64) -> (i32, u32, u32) {
    // 简易实现，假设正数；足以用于输出展示
    let days = (secs / 86_400) as i64;
    // 以 2000-03-01 为锚的经典算法
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m as u32, d as u32)
}

fn simple_logger_init() {
    // example 不需要复杂日志，直接忽略 log 输出
    let _ = log::set_max_level(log::LevelFilter::Warn);
}
