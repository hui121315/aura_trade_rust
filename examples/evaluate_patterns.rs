//! 形态有效性评估脚本
//!
//! 用真实 Binance 历史 K 线（缓存读取）评估：
//!   - **K 线形态**：scan 出所有命中，检查命中后 N 根 K 线的方向一致性与平均收益
//!   - **技术图形**：同上，窗口稍长
//!   - **均线特殊形态**：在每个 bar 逐根扫描命中，再评估
//!
//! 指标：
//!   - n_hits  命中次数
//!   - hit_rate  预测方向正确的命中占比（0 ~ 1）
//!   - avg_return  命中后 N 根平均"定向收益" = direction * (close[+N] - close[0]) / close[0]
//!   - expectancy = hit_rate × avg_win − (1−hit_rate) × avg_loss  （仅方向胜负统计）
//!   - score = avg_return / return_stddev （类 Sharpe，越大越好）
//!
//! 用法：
//! ```bash
//! cargo run --example evaluate_patterns --release -- [symbol] [interval] [limit] [horizon]
//! # 例如：
//! cargo run --example evaluate_patterns --release -- BTCUSDT 4h 2000 5
//! ```
//!
//! horizon = 命中后评估的 K 线根数（默认 5）

use std::collections::HashMap;

use aura_trade::config::Config;
use aura_trade::data::{Binance, Bitget, Bybit, KlineCache, Okx, Timeframe};
use aura_trade::engine::candle::{self, PatternKind};
use aura_trade::engine::chartpattern::{self, ChartPatternKind};
use aura_trade::engine::ma::{self, alignment, compute, MaKind, MaSpecialKind, SpecialParams};

#[derive(Default, Clone)]
struct Stat {
    n_hits: usize,
    correct: usize,               // 方向正确
    returns: Vec<f64>,            // 每次命中后 N 根的定向收益
    direction: i32,               // 主方向：+1/-1/0（多数命中的方向）
    dir_sum: i32,                 // sum(direction) 用于推断
}

impl Stat {
    fn add(&mut self, correct: bool, ret: f64, direction: i8) {
        self.n_hits += 1;
        if correct { self.correct += 1; }
        self.returns.push(ret);
        self.dir_sum += direction as i32;
        self.direction = if self.dir_sum > 0 { 1 } else if self.dir_sum < 0 { -1 } else { 0 };
    }
    fn hit_rate(&self) -> f64 {
        if self.n_hits == 0 { 0.0 } else { self.correct as f64 / self.n_hits as f64 }
    }
    fn avg_return(&self) -> f64 {
        if self.returns.is_empty() { 0.0 } else {
            self.returns.iter().sum::<f64>() / self.returns.len() as f64
        }
    }
    fn std(&self) -> f64 {
        if self.returns.len() < 2 { return 0.0; }
        let m = self.avg_return();
        let var: f64 = self.returns.iter().map(|r| (r - m).powi(2)).sum::<f64>()
            / (self.returns.len() - 1) as f64;
        var.sqrt()
    }
    fn score(&self) -> f64 {
        let s = self.std();
        if s < 1e-9 { 0.0 } else { self.avg_return() / s }
    }
}

/// 评分等级
fn rank(stat: &Stat, alpha: f64, min_hits: usize) -> &'static str {
    if stat.n_hits < min_hits { return "样本不足"; }
    let hr = stat.hit_rate();
    if alpha > 0.008 && hr >= 0.58 { "强可用 ★★★" }
    else if alpha > 0.004 && hr >= 0.54 { "可用 ★★" }
    else if alpha > 0.001 && hr >= 0.51 { "一般 ★" }
    else if alpha.abs() < 0.001 || (hr - 0.5).abs() < 0.02 { "无偏" }
    else { "反向失效" }
}

/// 计算形态的"**方向感知 alpha**"
///   多头形态（direction=+1）：alpha = avg_ret - mu
///   空头形态（direction=-1）：alpha = avg_ret - (-mu) = avg_ret + mu
///   中性形态：alpha = avg_ret（期望为 0）
fn alpha_of(stat: &Stat, mu: f64) -> f64 {
    let avg = stat.avg_return();
    match stat.direction {
        1 => avg - mu,
        -1 => avg + mu,
        _ => avg,
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let symbol = args.get(1).map(String::as_str).unwrap_or("BTCUSDT").to_string();
    let interval_s = args.get(2).map(String::as_str).unwrap_or("4h").to_string();
    let limit: usize = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(2000);
    let horizon: usize = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(5);

    let tf = Timeframe::parse(&interval_s).expect("invalid interval");

    // 拉数据（用默认配置 + 环境变量覆盖）
    let cfg = Config::from_env();
    let cache = KlineCache::new(
        cfg.cache_dir.clone(),
        Binance::new(cfg.binance_base.clone()),
        Bybit::new("https://api.bybit.com"),
        Bitget::new("https://api.bitget.com"),
        Okx::new("https://www.okx.com"),
    )
        .with_ttl(300); // 评估脚本可接受较长缓存
    println!("📥 加载 {} {} × {} 根 K 线 ...", symbol, tf.as_str(), limit);
    let klines = cache.get(&symbol, tf, limit).expect("fetch klines");
    println!("   实际获得 {} 根（{} → {}）\n", klines.len(),
        fmt_ts(klines.first().unwrap().open_time),
        fmt_ts(klines.last().unwrap().open_time));

    let closes: Vec<f64> = klines.iter().map(|k| k.close).collect();

    // ==================== 市场漂移 ====================
    // mu = 市场 N 根后的平均"**有向**"收益；形态多头 alpha 对比 mu，空头对比 -mu
    let chart_horizon = horizon * 2;
    let mut mkt: Vec<f64> = Vec::new();
    for i in 0..(closes.len().saturating_sub(horizon)) {
        mkt.push((closes[i + horizon] - closes[i]) / closes[i]);
    }
    let mu: f64 = if mkt.is_empty() { 0.0 } else { mkt.iter().sum::<f64>() / mkt.len() as f64 };
    let mut mkt_c: Vec<f64> = Vec::new();
    for i in 0..(closes.len().saturating_sub(chart_horizon)) {
        mkt_c.push((closes[i + chart_horizon] - closes[i]) / closes[i]);
    }
    let mu_chart: f64 = if mkt_c.is_empty() { 0.0 } else { mkt_c.iter().sum::<f64>() / mkt_c.len() as f64 };

    // 整体趋势
    let total_ret = (closes.last().unwrap() - closes.first().unwrap()) / closes.first().unwrap();

    // ==================== K 线形态 ====================
    let candle_hits = candle::scan(&klines);
    let mut candle_stats: HashMap<PatternKind, Stat> = HashMap::new();
    for h in &candle_hits {
        let idx = h.index;
        if idx + horizon >= closes.len() { continue; }
        let future = closes[idx + horizon];
        let now = closes[idx];
        let raw_ret = (future - now) / now;
        let dir_ret = (h.direction as f64) * raw_ret;
        let correct = match h.direction {
            d if d > 0 => future > now,
            d if d < 0 => future < now,
            _ => raw_ret.abs() < 0.003, // 中性形态：价格不大变视为正确
        };
        candle_stats.entry(h.kind).or_default().add(correct, dir_ret, h.direction);
    }

    // ==================== 技术图形 ====================
    let chart_hits = chartpattern::detect_all(&klines);
    let mut chart_stats: HashMap<ChartPatternKind, Stat> = HashMap::new();
    for p in &chart_hits {
        let idx = p.completion_index;
        if idx + chart_horizon >= closes.len() { continue; }
        let future = closes[idx + chart_horizon];
        let now = closes[idx];
        let raw_ret = (future - now) / now;
        let dir_ret = (p.direction as f64) * raw_ret;
        let correct = match p.direction {
            d if d > 0 => future > now,
            d if d < 0 => future < now,
            _ => raw_ret.abs() < 0.003,
        };
        chart_stats.entry(p.kind).or_default().add(correct, dir_ret, p.direction);
    }

    // ==================== 均线特殊形态 ====================
    //   对每根 bar 调用 scan_at（只生成当根的命中），然后评估后续 N 根
    let periods: Vec<usize> = vec![5, 10, 20, 30, 60, 120, 250];
    let ma_kind = MaKind::Sma;
    let ma_series: Vec<Vec<f64>> = periods.iter().map(|&p| compute::compute(ma_kind, &closes, p)).collect();
    let slope_base = compute::slope(&ma_series[periods.iter().position(|&p| p == 30).unwrap()], 5);
    // 计算所有相邻均线交叉索引
    let mut cross_bars: Vec<usize> = Vec::new();
    for i in 0..periods.len() - 1 {
        for c in alignment::find_crosses(&ma_series[i], &ma_series[i + 1], periods[i], periods[i + 1]) {
            cross_bars.push(c.index);
        }
    }
    cross_bars.sort();
    cross_bars.dedup();

    let params = SpecialParams::default();
    let mut ma_stats: HashMap<MaSpecialKind, Stat> = HashMap::new();
    // 从 MA 稳定位置起扫描（跳过前 250 根让 MA250 就绪）
    for i in 260..(closes.len().saturating_sub(horizon + 1)) {
        // 取当前位置的 alignment
        let stack_refs: Vec<&[f64]> = ma_series.iter().map(|v| v.as_slice()).collect();
        let alignment_i = alignment::classify(&stack_refs, i, 0.005);

        // 仅保留 ≤ 当前索引的 cross_bars（模拟实时）
        let crosses_up_to_i: Vec<usize> = cross_bars.iter().cloned().filter(|&x| x <= i).collect();

        let hits = ma::scan_ma_special(
            &closes, &ma_series, &periods, alignment_i,
            &slope_base, 30, &crosses_up_to_i, i, &params,
        );
        if hits.is_empty() { continue; }
        let future = closes[i + horizon];
        let now = closes[i];
        let raw_ret = (future - now) / now;
        for h in hits {
            let dir_ret = (h.direction as f64) * raw_ret;
            let correct = match h.direction {
                d if d > 0 => future > now,
                d if d < 0 => future < now,
                _ => raw_ret.abs() < 0.003,
            };
            ma_stats.entry(h.kind).or_default().add(correct, dir_ret, h.direction);
        }
    }

    // ==================== 打印报告 ====================
    println!("{}", "=".repeat(102));
    println!("形态有效性评估  |  {} {}  |  K 线 {} 根  |  K线horizon = {} 根  |  图形horizon = {} 根",
        symbol, tf.as_str(), klines.len(), horizon, chart_horizon);
    println!("区间首尾涨跌: {:+.2}%  |  市场平均漂移 μ：K线 {:+.3}% / 图形 {:+.3}% （含符号，做多基准=μ，做空基准=-μ）",
        total_ret * 100.0, mu * 100.0, mu_chart * 100.0);
    println!("{}\n", "=".repeat(102));

    print_candle_section(&candle_stats, mu);
    println!();
    print_chart_section(&chart_stats, chart_horizon, mu_chart);
    println!();
    print_ma_section(&ma_stats, mu);
    println!();
    print_summary();
}

/// 把中文字符串填充到显示宽度（中文 ×2，英文 ×1）
fn pad(s: &str, width: usize) -> String {
    let w: usize = s.chars().map(|c| if (c as u32) > 127 { 2 } else { 1 }).sum();
    if w >= width { s.to_string() } else { format!("{}{}", s, " ".repeat(width - w)) }
}

fn dir_mark(d: i32) -> &'static str {
    match d { 1 => "多", -1 => "空", _ => "中" }
}

fn print_candle_section(stats: &HashMap<PatternKind, Stat>, mu: f64) {
    println!("K 线形态（46+ 种）—— 历史触发 {} 种", stats.len());
    println!("{}", "-".repeat(102));
    println!("{} {:>3} {:>6} {:>9} {:>11} {:>11} {:>10}  {}",
        pad("形态", 22), "向", "命中", "胜率", "平均收益", "alpha", "Sharpe", "评级");
    let mut rows: Vec<(PatternKind, Stat)> = stats.iter().map(|(k, v)| (*k, v.clone())).collect();
    rows.sort_by(|a, b| alpha_of(&b.1, mu).partial_cmp(&alpha_of(&a.1, mu)).unwrap_or(std::cmp::Ordering::Equal));
    for (k, s) in rows {
        let alpha = alpha_of(&s, mu);
        println!("{} {:>3} {:>6} {:>8.1}% {:>10.3}% {:>10.3}% {:>10.3}  {}",
            pad(k.label(), 22), dir_mark(s.direction), s.n_hits, s.hit_rate() * 100.0,
            s.avg_return() * 100.0, alpha * 100.0, s.score(), rank(&s, alpha, 10));
    }
}

fn print_chart_section(stats: &HashMap<ChartPatternKind, Stat>, horizon: usize, mu: f64) {
    println!("技术图形（25 种，horizon = {}）—— 历史触发 {} 种", horizon, stats.len());
    println!("{}", "-".repeat(102));
    println!("{} {:>3} {:>6} {:>9} {:>11} {:>11} {:>10}  {}",
        pad("图形", 22), "向", "命中", "胜率", "平均收益", "alpha", "Sharpe", "评级");
    let mut rows: Vec<(ChartPatternKind, Stat)> = stats.iter().map(|(k, v)| (*k, v.clone())).collect();
    rows.sort_by(|a, b| alpha_of(&b.1, mu).partial_cmp(&alpha_of(&a.1, mu)).unwrap_or(std::cmp::Ordering::Equal));
    for (k, s) in rows {
        let alpha = alpha_of(&s, mu);
        println!("{} {:>3} {:>6} {:>8.1}% {:>10.3}% {:>10.3}% {:>10.3}  {}",
            pad(k.label(), 22), dir_mark(s.direction), s.n_hits, s.hit_rate() * 100.0,
            s.avg_return() * 100.0, alpha * 100.0, s.score(), rank(&s, alpha, 3));
    }
}

fn print_ma_section(stats: &HashMap<MaSpecialKind, Stat>, mu: f64) {
    println!("均线特殊形态（17 种）—— 历史触发 {} 种", stats.len());
    println!("{}", "-".repeat(102));
    println!("{} {:>3} {:>6} {:>9} {:>11} {:>11} {:>10}  {}",
        pad("形态", 22), "向", "命中", "胜率", "平均收益", "alpha", "Sharpe", "评级");
    let mut rows: Vec<(MaSpecialKind, Stat)> = stats.iter().map(|(k, v)| (*k, v.clone())).collect();
    rows.sort_by(|a, b| alpha_of(&b.1, mu).partial_cmp(&alpha_of(&a.1, mu)).unwrap_or(std::cmp::Ordering::Equal));
    for (k, s) in rows {
        let alpha = alpha_of(&s, mu);
        println!("{} {:>3} {:>6} {:>8.1}% {:>10.3}% {:>10.3}% {:>10.3}  {}",
            pad(k.label(), 22), dir_mark(s.direction), s.n_hits, s.hit_rate() * 100.0,
            s.avg_return() * 100.0, alpha * 100.0, s.score(), rank(&s, alpha, 20));
    }
}

fn print_summary() {
    println!("{}", "=".repeat(102));
    println!("评级口径（alpha = 形态平均定向收益 - 同期基准 |绝对| 收益）：");
    println!("  强可用 ★★★  alpha > 0.8% 且 胜率 ≥ 58%");
    println!("  可用 ★★    alpha > 0.4% 且 胜率 ≥ 54%");
    println!("  一般 ★     alpha > 0.1% 且 胜率 ≥ 51%");
    println!("  无偏       alpha ≈ 0 或 胜率 ≈ 50%（形态在该标的上无预测力）");
    println!("  反向失效   alpha < 0 且 胜率 < 50%，形态方向与实际反向");
    println!("  样本不足   命中次数 < 阈值（K线 10 / 图形 3 / 均线 20）");
    println!();
    println!("说明：");
    println!("  - 平均收益 = direction × (close[+N] - close[0]) / close[0]，方向正确为正");
    println!("  - 基准    = 所有 K 线后 N 根的平均 |收益|（多空对称基线）");
    println!("  - alpha   = 平均收益 - 基准，衡量形态是否具备超过'随机入场'的预测力");
}

fn fmt_ts(ms: i64) -> String {
    use std::time::{UNIX_EPOCH, Duration};
    let d = UNIX_EPOCH + Duration::from_millis(ms as u64);
    // 简单格式化：用 chrono 太重；手工算 YYYY-MM-DD
    let secs = d.duration_since(UNIX_EPOCH).map(|x| x.as_secs()).unwrap_or(0) as i64;
    let days = secs / 86400;
    // Unix epoch 1970-01-01 = day 0
    let (y, m, d) = days_to_ymd(days);
    format!("{:04}-{:02}-{:02}", y, m, d)
}

fn days_to_ymd(days: i64) -> (i32, u32, u32) {
    // from civil_from_days algorithm (Howard Hinnant)
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe/1460 + doe/36524 - doe/146096) / 365;
    let y = yoe as i32 + era as i32 * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}
