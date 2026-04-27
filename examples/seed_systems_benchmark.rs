//! 种子体系横向基准（M3 新增）
//!
//! 把全部 8 个种子体系跑在 BTC/ETH/SOL × 指定周期上，生成矩阵报告。
//!
//! # 用法
//!
//! ```bash
//! cargo run --release --example seed_systems_benchmark                     # 默认 1d 1000 根
//! cargo run --release --example seed_systems_benchmark -- 4h 2000          # 自定义周期/根数
//! cargo run --release --example seed_systems_benchmark -- 1d 1000 BTC,ETH  # 自定义 symbol 列表
//! ```
//!
//! # 输出
//!
//! - 第一段：每个 symbol 的 21 组件触发次数概览（识别本段数据的"活力"）
//! - 第二段：所有种子体系 × 所有 symbol 的矩阵表格
//!   - 胜率 / 总收益 / Sharpe / 最大回撤
//! - 第三段：每个种子体系的"综合名次"（跨 symbol 平均 Sharpe 排序）

use std::env;
use std::time::Instant;

use aura_trade::data::binance::Binance;
use aura_trade::data::Timeframe;
use aura_trade::engine::system::{self, all_seeds};

#[derive(Debug, Clone)]
struct Row {
    seed_id: String,
    seed_name: String,
    per_symbol: Vec<PerSymbol>,
}

#[derive(Debug, Clone)]
struct PerSymbol {
    symbol: String,
    trades: usize,
    win_rate: f64,
    total_return: f64,
    sharpe: f64,
    max_dd: f64,
}

fn main() {
    simple_logger_init();

    let args: Vec<String> = env::args().collect();
    let interval_str = args.get(1).cloned().unwrap_or_else(|| "1d".into());
    let limit: usize = args
        .get(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(1000);
    let symbols: Vec<String> = args
        .get(3)
        .cloned()
        .unwrap_or_else(|| "BTC,ETH,SOL".into())
        .split(',')
        .map(|s| format!("{}USDT", s.trim().to_uppercase()))
        .collect();

    let tf = Timeframe::parse(&interval_str).unwrap_or_else(|| {
        eprintln!("非法周期 `{}`", interval_str);
        std::process::exit(1);
    });

    println!("🏁  种子体系横向基准  [{} 体系 × {} symbol × {} 根 {}]",
        all_seeds().len(), symbols.len(), limit, interval_str);
    println!("═══════════════════════════════════════════════════════════");

    let client = Binance::new("https://api.binance.com");
    let mut data_per_symbol: Vec<(String, Vec<aura_trade::data::Kline>)> = Vec::new();
    for sym in &symbols {
        print!("📥  拉取 {} {} {} 根 ... ", sym, interval_str, limit);
        let t = Instant::now();
        match client.klines(sym, tf, limit) {
            Ok(klines) => {
                println!("✓ {} 根  [{:.1}s]", klines.len(), t.elapsed().as_secs_f64());
                data_per_symbol.push((sym.clone(), klines));
            }
            Err(e) => {
                println!("✗ {}", e);
            }
        }
    }
    println!();

    if data_per_symbol.is_empty() {
        eprintln!("❌ 所有 symbol 都拉取失败。退出。");
        std::process::exit(2);
    }

    // --------- 跑矩阵 ----------
    let seeds = all_seeds();
    let mut rows: Vec<Row> = Vec::with_capacity(seeds.len());
    let t0 = Instant::now();

    for def in &seeds {
        let mut row = Row {
            seed_id: def.id.clone(),
            seed_name: def.name.clone(),
            per_symbol: Vec::new(),
        };
        for (sym, klines) in &data_per_symbol {
            match system::run(def, klines, sym, &interval_str) {
                Ok(r) => {
                    let p = &r.performance;
                    row.per_symbol.push(PerSymbol {
                        symbol: sym.clone(),
                        trades: p.total_trades,
                        win_rate: p.win_rate,
                        total_return: p.total_return_pct,
                        sharpe: p.sharpe,
                        max_dd: p.max_drawdown_pct,
                    });
                }
                Err(e) => {
                    eprintln!("  ⚠ {} @ {} 失败: {}", def.id, sym, e);
                    row.per_symbol.push(PerSymbol {
                        symbol: sym.clone(),
                        trades: 0,
                        win_rate: 0.0,
                        total_return: 0.0,
                        sharpe: 0.0,
                        max_dd: 0.0,
                    });
                }
            }
        }
        rows.push(row);
    }
    let elapsed = t0.elapsed();
    println!("✓ 矩阵完成 [{:.1} ms, {} 次回测]",
        elapsed.as_secs_f64() * 1000.0, seeds.len() * data_per_symbol.len());
    println!();

    // --------- 打印矩阵 ----------
    print_matrix(&rows, &data_per_symbol.iter().map(|(s, _)| s.clone()).collect::<Vec<_>>());

    // --------- 综合排名（跨 symbol 平均 Sharpe）----------
    println!();
    println!("🏆  跨 symbol 综合排名（按平均 Sharpe 降序）:");
    println!("──────────────────────────────────────────────────────");
    let mut ranked: Vec<(String, String, f64, f64, f64)> = rows
        .iter()
        .map(|row| {
            let n = row.per_symbol.len() as f64;
            let avg_sharpe: f64 =
                row.per_symbol.iter().map(|p| p.sharpe).sum::<f64>() / n.max(1.0);
            let avg_ret: f64 =
                row.per_symbol.iter().map(|p| p.total_return).sum::<f64>() / n.max(1.0);
            let avg_dd: f64 =
                row.per_symbol.iter().map(|p| p.max_dd).sum::<f64>() / n.max(1.0);
            (row.seed_id.clone(), row.seed_name.clone(), avg_sharpe, avg_ret, avg_dd)
        })
        .collect();
    ranked.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

    println!(
        "  {:<6} {:<28} {:<18} {:>11} {:>11} {:>11}",
        "#", "体系 ID", "名称", "Avg-Sharpe", "Avg-Ret", "Avg-DD"
    );
    for (i, (id, name, sharpe, ret, dd)) in ranked.iter().enumerate() {
        let trophy = match i {
            0 => "🥇",
            1 => "🥈",
            2 => "🥉",
            _ => "  ",
        };
        println!(
            "  {} {:<2} {:<28} {:<18} {:>+10.2}  {:>+9.2}%  {:>9.2}%",
            trophy,
            i + 1,
            id,
            truncate(name, 18),
            sharpe,
            ret * 100.0,
            dd * 100.0,
        );
    }
    println!();
    println!("✅ 基准完成。");
}

fn print_matrix(rows: &[Row], symbols: &[String]) {
    // 按 symbol 分组展示 4 个指标
    for metric in ["胜率", "总收益", "Sharpe", "最大回撤"] {
        println!("── {} 矩阵 ─────────────────────────────────────────", metric);
        print!("  {:<28}", "体系");
        for s in symbols {
            print!("{:>12}", s);
        }
        println!();
        for row in rows {
            print!("  {:<28}", truncate(&row.seed_id, 28));
            for p in &row.per_symbol {
                let cell = match metric {
                    "胜率" => format!("{:.1}%/{}", p.win_rate * 100.0, p.trades),
                    "总收益" => format!("{:+.2}%", p.total_return * 100.0),
                    "Sharpe" => format!("{:+.2}", p.sharpe),
                    "最大回撤" => format!("{:.2}%", p.max_dd * 100.0),
                    _ => String::new(),
                };
                print!("{:>12}", cell);
            }
            println!();
        }
        println!();
    }
}

fn truncate(s: &str, max: usize) -> String {
    let mut out = String::new();
    let mut cnt = 0;
    for ch in s.chars() {
        // 粗略：ASCII 占 1，其他占 2
        let w = if ch.is_ascii() { 1 } else { 2 };
        if cnt + w > max {
            break;
        }
        out.push(ch);
        cnt += w;
    }
    out
}

fn simple_logger_init() {
    // 与 system_backtest.rs 保持一致的极简初始化（让 aura_trade 内部 log 静音）
    std::env::set_var("RUST_LOG", "warn");
}
