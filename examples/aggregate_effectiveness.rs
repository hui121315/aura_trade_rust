//! 跨多数据集聚合形态有效性
//!
//! 运行多个 symbol × timeframe 组合，把结果合并成一份"最终排行榜"：
//!   - K 线形态：合并命中 + 平均胜率 + 跨数据集 alpha 均值 + 稳定性(σ)
//!   - 技术图形：同上
//!   - 均线特殊形态：同上
//!
//! 数据集列表（代表不同市场状态）：
//!   1. BTC 1d  × 1500 （约 4 年，完整牛熊）
//!   2. BTC 4h  × 2000 （近 1 年，下跌）
//!   3. BTC 1h  × 5000 （近半年，震荡/下跌）
//!   4. ETH 4h  × 2000 （近 1 年，震荡）
//!   5. SOL 4h  × 2000 （近 1 年，震荡）

use std::collections::HashMap;

use aura_trade::config::Config;
use aura_trade::data::{Binance, Bitget, Bybit, KlineCache, Okx, Timeframe};
use aura_trade::engine::candle::{self, PatternKind};
use aura_trade::engine::chartpattern::{self, ChartPatternKind};
use aura_trade::engine::ma::{self, alignment, compute, MaKind, MaSpecialKind, SpecialParams};

#[derive(Default, Clone)]
struct Hit {
    ret: f64,
    correct: bool,
    direction: i8,
    dataset: String,
}

fn main() {
    // 覆盖 周线 / 日线 / 4h 三种时间框架 × 三个主流标的
    //   周线：Binance 从 2017 起，理论上能拿约 500 根（10 年）
    //   日线：1500 根 ≈ 4 年
    //   4h：2000 根 ≈ 11 个月
    let datasets: Vec<(&str, &str, usize)> = vec![
        ("BTCUSDT", "1w", 500),
        ("ETHUSDT", "1w", 500),
        ("SOLUSDT", "1w", 500),
        ("BTCUSDT", "1d", 1500),
        ("ETHUSDT", "1d", 1500),
        ("SOLUSDT", "1d", 1500),
        ("BTCUSDT", "4h", 2000),
        ("ETHUSDT", "4h", 2000),
        ("SOLUSDT", "4h", 2000),
    ];
    let horizon: usize = 5;
    let chart_horizon = horizon * 2;

    let cfg = Config::from_env();
    let cache = KlineCache::new(
        cfg.cache_dir.clone(),
        Binance::new(cfg.binance_base.clone()),
        Bybit::new("https://api.bybit.com"),
        Bitget::new("https://api.bitget.com"),
        Okx::new("https://www.okx.com"),
    )
        .with_ttl(600);

    let mut all_candle: HashMap<PatternKind, Vec<Hit>> = HashMap::new();
    let mut all_chart: HashMap<ChartPatternKind, Vec<Hit>> = HashMap::new();
    let mut all_ma: HashMap<MaSpecialKind, Vec<Hit>> = HashMap::new();
    let mut dataset_labels: Vec<String> = Vec::new();
    let mut mu_per_ds: Vec<f64> = Vec::new();       // K 线 horizon 市场漂移
    let mut mu_chart_per_ds: Vec<f64> = Vec::new(); // 图形 horizon 市场漂移

    for (sym, intv, limit) in &datasets {
        let tf = Timeframe::parse(intv).expect("tf");
        println!("📥 {} {} × {} ...", sym, intv, limit);
        let klines = match cache.get(sym, tf, *limit) {
            Ok(v) => v,
            Err(e) => { println!("   跳过（{}）", e); continue; }
        };
        let closes: Vec<f64> = klines.iter().map(|k| k.close).collect();
        let label = format!("{}-{}", sym, intv);
        dataset_labels.push(label.clone());

        // market drift
        let mu: f64 = {
            let rs: Vec<f64> = (0..closes.len().saturating_sub(horizon))
                .map(|i| (closes[i + horizon] - closes[i]) / closes[i]).collect();
            if rs.is_empty() { 0.0 } else { rs.iter().sum::<f64>() / rs.len() as f64 }
        };
        let mu_c: f64 = {
            let rs: Vec<f64> = (0..closes.len().saturating_sub(chart_horizon))
                .map(|i| (closes[i + chart_horizon] - closes[i]) / closes[i]).collect();
            if rs.is_empty() { 0.0 } else { rs.iter().sum::<f64>() / rs.len() as f64 }
        };
        mu_per_ds.push(mu);
        mu_chart_per_ds.push(mu_c);

        // 候选形态扫描
        for h in candle::scan(&klines) {
            if h.index + horizon >= closes.len() { continue; }
            let now = closes[h.index]; let fut = closes[h.index + horizon];
            let raw = (fut - now) / now;
            let dir_ret = (h.direction as f64) * raw;
            let correct = match h.direction { d if d > 0 => fut > now, d if d < 0 => fut < now, _ => raw.abs() < 0.003 };
            all_candle.entry(h.kind).or_default().push(Hit {
                ret: dir_ret, correct, direction: h.direction, dataset: label.clone(),
            });
        }
        for p in chartpattern::detect_all(&klines) {
            if p.completion_index + chart_horizon >= closes.len() { continue; }
            let now = closes[p.completion_index]; let fut = closes[p.completion_index + chart_horizon];
            let raw = (fut - now) / now;
            let dir_ret = (p.direction as f64) * raw;
            let correct = match p.direction { d if d > 0 => fut > now, d if d < 0 => fut < now, _ => raw.abs() < 0.003 };
            all_chart.entry(p.kind).or_default().push(Hit {
                ret: dir_ret, correct, direction: p.direction, dataset: label.clone(),
            });
        }

        // 均线特殊形态
        let periods: Vec<usize> = vec![5, 10, 20, 30, 60, 120, 250];
        let ma_series: Vec<Vec<f64>> = periods.iter().map(|&p| compute::compute(MaKind::Sma, &closes, p)).collect();
        let slope_base = compute::slope(&ma_series[periods.iter().position(|&p| p == 30).unwrap()], 5);
        let mut cross_bars: Vec<usize> = Vec::new();
        for i in 0..periods.len() - 1 {
            for c in alignment::find_crosses(&ma_series[i], &ma_series[i + 1], periods[i], periods[i + 1]) {
                cross_bars.push(c.index);
            }
        }
        cross_bars.sort(); cross_bars.dedup();
        let params = SpecialParams::default();
        for i in 260..closes.len().saturating_sub(horizon + 1) {
            let stack_refs: Vec<&[f64]> = ma_series.iter().map(|v| v.as_slice()).collect();
            let alg = alignment::classify(&stack_refs, i, 0.005);
            let crosses_i: Vec<usize> = cross_bars.iter().cloned().filter(|&x| x <= i).collect();
            let hits = ma::scan_ma_special(&closes, &ma_series, &periods, alg, &slope_base, 30, &crosses_i, i, &params);
            if hits.is_empty() { continue; }
            let now = closes[i]; let fut = closes[i + horizon];
            let raw = (fut - now) / now;
            for h in hits {
                let dir_ret = (h.direction as f64) * raw;
                let correct = match h.direction { d if d > 0 => fut > now, d if d < 0 => fut < now, _ => raw.abs() < 0.003 };
                all_ma.entry(h.kind).or_default().push(Hit {
                    ret: dir_ret, correct, direction: h.direction, dataset: label.clone(),
                });
            }
        }
    }

    // ======== 全局聚合 ========
    println!("\n{}", "=".repeat(118));
    println!("📊 跨数据集全量聚合  |  共 {} 个数据集  |  horizon = {}（K线）/ {}（图形）",
        dataset_labels.len(), horizon, chart_horizon);
    println!("数据集: {}", dataset_labels.join(", "));
    println!("{}", "=".repeat(118));
    println!();

    print_candles(&all_candle, &dataset_labels, &mu_per_ds, "全量");
    println!();
    print_charts(&all_chart, &dataset_labels, &mu_chart_per_ds, "全量");
    println!();
    print_mas(&all_ma, &dataset_labels, &mu_per_ds, "全量");

    // ======== 按时间框架分组 ========
    for tf_tag in &["1w", "1d", "4h"] {
        println!("\n{}", "=".repeat(118));
        println!("📂 时间框架分组：{} （只聚合该周期的数据集）", tf_tag);
        let ds_idx: Vec<usize> = dataset_labels.iter().enumerate()
            .filter_map(|(i, l)| if l.ends_with(&format!("-{}", tf_tag)) { Some(i) } else { None })
            .collect();
        if ds_idx.is_empty() { println!("   无数据"); continue; }
        let sub_labels: Vec<String> = ds_idx.iter().map(|&i| dataset_labels[i].clone()).collect();
        let sub_mu: Vec<f64> = ds_idx.iter().map(|&i| mu_per_ds[i]).collect();
        let sub_mu_chart: Vec<f64> = ds_idx.iter().map(|&i| mu_chart_per_ds[i]).collect();
        println!("数据集: {}", sub_labels.join(", "));
        println!("{}", "=".repeat(118));
        let sub_candle: HashMap<PatternKind, Vec<Hit>> = all_candle.iter()
            .map(|(k, v)| (*k, v.iter().filter(|h| sub_labels.contains(&h.dataset)).cloned().collect::<Vec<_>>()))
            .filter(|(_, v)| !v.is_empty()).collect();
        let sub_chart: HashMap<ChartPatternKind, Vec<Hit>> = all_chart.iter()
            .map(|(k, v)| (*k, v.iter().filter(|h| sub_labels.contains(&h.dataset)).cloned().collect::<Vec<_>>()))
            .filter(|(_, v)| !v.is_empty()).collect();
        let sub_ma: HashMap<MaSpecialKind, Vec<Hit>> = all_ma.iter()
            .map(|(k, v)| (*k, v.iter().filter(|h| sub_labels.contains(&h.dataset)).cloned().collect::<Vec<_>>()))
            .filter(|(_, v)| !v.is_empty()).collect();

        println!();
        print_candles(&sub_candle, &sub_labels, &sub_mu, tf_tag);
        println!();
        print_charts(&sub_chart, &sub_labels, &sub_mu_chart, tf_tag);
        println!();
        print_mas(&sub_ma, &sub_labels, &sub_mu, tf_tag);
    }

    println!();
    print_legend();
}

// ---------- 打印辅助 ----------

fn pad(s: &str, width: usize) -> String {
    let w: usize = s.chars().map(|c| if (c as u32) > 127 { 2 } else { 1 }).sum();
    if w >= width { s.to_string() } else { format!("{}{}", s, " ".repeat(width - w)) }
}

fn dir_mark(d: i8) -> &'static str { match d { 1 => "多", -1 => "空", _ => "中" } }

fn summary_from_hits(hits: &[Hit], datasets: &[String], mu_per_ds: &[f64]) -> Summary {
    let mut s = Summary::default();
    if hits.is_empty() { return s; }
    s.n = hits.len();
    s.correct = hits.iter().filter(|h| h.correct).count();
    s.direction = {
        let sum: i32 = hits.iter().map(|h| h.direction as i32).sum();
        if sum > 0 { 1 } else if sum < 0 { -1 } else { 0 }
    };
    s.avg_ret = hits.iter().map(|h| h.ret).sum::<f64>() / hits.len() as f64;
    // 按数据集计算 alpha
    let mu_map: HashMap<&str, f64> = datasets.iter().map(String::as_str).zip(mu_per_ds.iter().copied()).collect();
    let mut per_ds: HashMap<&str, Vec<f64>> = HashMap::new();
    for h in hits {
        per_ds.entry(h.dataset.as_str()).or_default().push(h.ret);
    }
    let alphas: Vec<f64> = per_ds.iter().map(|(ds, rs)| {
        let avg = rs.iter().sum::<f64>() / rs.len() as f64;
        let m = *mu_map.get(ds).unwrap_or(&0.0);
        match s.direction { 1 => avg - m, -1 => avg + m, _ => avg }
    }).collect();
    s.alpha = if alphas.is_empty() { 0.0 } else { alphas.iter().sum::<f64>() / alphas.len() as f64 };
    s.n_datasets = per_ds.len();
    // 稳定性：跨数据集 alpha 的 std
    s.stability = {
        if alphas.len() < 2 { 0.0 } else {
            let m = s.alpha;
            let var = alphas.iter().map(|a| (a - m).powi(2)).sum::<f64>() / (alphas.len() - 1) as f64;
            var.sqrt()
        }
    };
    s.ds_positive = alphas.iter().filter(|a| **a > 0.0).count();
    s
}

#[derive(Default, Clone)]
struct Summary {
    n: usize,
    correct: usize,
    direction: i8,
    avg_ret: f64,
    alpha: f64,         // 跨数据集 alpha 均值
    stability: f64,     // 跨数据集 alpha σ
    n_datasets: usize,  // 出现的数据集数量
    ds_positive: usize, // alpha > 0 的数据集数量
}

impl Summary {
    fn hit_rate(&self) -> f64 { if self.n == 0 { 0.0 } else { self.correct as f64 / self.n as f64 } }
}

fn rank(s: &Summary, min_hits: usize, min_datasets: usize) -> &'static str {
    if s.n < min_hits || s.n_datasets < min_datasets { return "样本不足"; }
    let hr = s.hit_rate();
    // 一致性加权：跨 N 个数据集中有 K 个为正 alpha，一致性 = K/N
    let consistency = s.ds_positive as f64 / s.n_datasets as f64;
    if s.alpha > 0.006 && hr >= 0.56 && consistency >= 0.8 { "强可用 ★★★" }
    else if s.alpha > 0.003 && hr >= 0.53 && consistency >= 0.6 { "可用 ★★" }
    else if s.alpha > 0.001 && hr >= 0.51 && consistency >= 0.5 { "一般 ★" }
    else if s.alpha.abs() < 0.001 || (hr - 0.5).abs() < 0.02 { "无偏" }
    else { "反向失效" }
}

/// 根据数据集标签自适应 min_hits（周线样本少，4h 样本多）
fn min_hits_for(tag: &str, kind: &str) -> (usize, usize) {
    // (min_hits, min_datasets)
    match (tag, kind) {
        ("1w", "candle") => (5, 2), ("1w", "chart") => (1, 1), ("1w", "ma") => (15, 2),
        ("1d", "candle") => (12, 2), ("1d", "chart") => (3, 2), ("1d", "ma") => (30, 2),
        ("4h", "candle") => (20, 2), ("4h", "chart") => (5, 2), ("4h", "ma") => (60, 2),
        (_, "chart") => (8, 2),
        (_, "ma") => (80, 3),
        _ => (30, 3),  // candle 及兜底
    }
}

fn print_candles(hits: &HashMap<PatternKind, Vec<Hit>>, ds: &[String], mu: &[f64], tag: &str) {
    println!("━━━━ K 线形态 [{}] ━━━━  触发种数: {}", tag, hits.len());
    println!("{}", "-".repeat(118));
    println!("{} {:>3} {:>5} {:>5} {:>9} {:>11} {:>11} {:>9} {:>8}  {}",
        pad("形态", 22), "向", "数据", "命中", "胜率", "平均收益", "alpha", "稳定σ", "正向DS", "评级");
    let (min_h, min_d) = min_hits_for(tag, "candle");
    let mut rows: Vec<(PatternKind, Summary)> = hits.iter().map(|(k, v)| (*k, summary_from_hits(v, ds, mu))).collect();
    rows.sort_by(|a, b| b.1.alpha.partial_cmp(&a.1.alpha).unwrap_or(std::cmp::Ordering::Equal));
    for (k, s) in rows {
        println!("{} {:>3} {:>5} {:>5} {:>8.1}% {:>10.3}% {:>10.3}% {:>8.3}% {:>2}/{:<2}   {}",
            pad(k.label(), 22), dir_mark(s.direction), s.n_datasets, s.n, s.hit_rate() * 100.0,
            s.avg_ret * 100.0, s.alpha * 100.0, s.stability * 100.0,
            s.ds_positive, s.n_datasets, rank(&s, min_h, min_d));
    }
}

fn print_charts(hits: &HashMap<ChartPatternKind, Vec<Hit>>, ds: &[String], mu: &[f64], tag: &str) {
    println!("━━━━ 技术图形 [{}] ━━━━  触发种数: {}", tag, hits.len());
    println!("{}", "-".repeat(118));
    println!("{} {:>3} {:>5} {:>5} {:>9} {:>11} {:>11} {:>9} {:>8}  {}",
        pad("图形", 22), "向", "数据", "命中", "胜率", "平均收益", "alpha", "稳定σ", "正向DS", "评级");
    let (min_h, min_d) = min_hits_for(tag, "chart");
    let mut rows: Vec<(ChartPatternKind, Summary)> = hits.iter().map(|(k, v)| (*k, summary_from_hits(v, ds, mu))).collect();
    rows.sort_by(|a, b| b.1.alpha.partial_cmp(&a.1.alpha).unwrap_or(std::cmp::Ordering::Equal));
    for (k, s) in rows {
        println!("{} {:>3} {:>5} {:>5} {:>8.1}% {:>10.3}% {:>10.3}% {:>8.3}% {:>2}/{:<2}   {}",
            pad(k.label(), 22), dir_mark(s.direction), s.n_datasets, s.n, s.hit_rate() * 100.0,
            s.avg_ret * 100.0, s.alpha * 100.0, s.stability * 100.0,
            s.ds_positive, s.n_datasets, rank(&s, min_h, min_d));
    }
}

fn print_mas(hits: &HashMap<MaSpecialKind, Vec<Hit>>, ds: &[String], mu: &[f64], tag: &str) {
    println!("━━━━ 均线特殊形态 [{}] ━━━━  触发种数: {}", tag, hits.len());
    println!("{}", "-".repeat(118));
    println!("{} {:>3} {:>5} {:>5} {:>9} {:>11} {:>11} {:>9} {:>8}  {}",
        pad("形态", 22), "向", "数据", "命中", "胜率", "平均收益", "alpha", "稳定σ", "正向DS", "评级");
    let (min_h, min_d) = min_hits_for(tag, "ma");
    let mut rows: Vec<(MaSpecialKind, Summary)> = hits.iter().map(|(k, v)| (*k, summary_from_hits(v, ds, mu))).collect();
    rows.sort_by(|a, b| b.1.alpha.partial_cmp(&a.1.alpha).unwrap_or(std::cmp::Ordering::Equal));
    for (k, s) in rows {
        println!("{} {:>3} {:>5} {:>5} {:>8.1}% {:>10.3}% {:>10.3}% {:>8.3}% {:>2}/{:<2}   {}",
            pad(k.label(), 22), dir_mark(s.direction), s.n_datasets, s.n, s.hit_rate() * 100.0,
            s.avg_ret * 100.0, s.alpha * 100.0, s.stability * 100.0,
            s.ds_positive, s.n_datasets, rank(&s, min_h, min_d));
    }
}

fn print_legend() {
    println!("{}", "=".repeat(118));
    println!("评级口径（alpha = 形态平均定向收益 - 同期市场漂移；在多个数据集上平均）：");
    println!("  强可用 ★★★ alpha > 0.6%, 胜率 ≥ 56%, 至少 80% 数据集正 alpha");
    println!("  可用 ★★    alpha > 0.3%, 胜率 ≥ 53%, 至少 60% 数据集正 alpha");
    println!("  一般 ★     alpha > 0.1%, 胜率 ≥ 51%, 至少 50% 数据集正 alpha");
    println!("  无偏       alpha ≈ 0 或 胜率 ≈ 50%，无预测力");
    println!("  反向失效   alpha < 0 且 胜率 < 50%，信号方向与实际反向");
    println!("  样本不足   命中或出现数据集不足（K线 30/3 图形 8/2 均线 80/3）");
    println!();
    println!("字段：");
    println!("  - 数据  = 该形态在多少个数据集中出现");
    println!("  - 正向DS = alpha > 0 的数据集数 / 总数据集数");
    println!("  - 稳定σ = 跨数据集 alpha 的标准差，越小越稳定");
}
