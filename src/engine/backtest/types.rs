//! 回测核心类型（对应 PRD §E1, §E4, §E5）

use serde::{Deserialize, Serialize};

use crate::data::Timeframe;
use crate::engine::ma::MaKind;

/// 方向
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Side {
    Long,
    Short,
}

/// 止损算法（PRD §4.2）
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum StopKind {
    /// 固定 N × ATR
    Atr,
    /// 前一波低点/高点 ± 缓冲
    Structure,
    /// 跌破关键均线
    Ma,
    /// 形态极值
    Pattern,
}

/// 回测配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestConfig {
    pub symbol: String,
    pub interval: String,
    /// 参与回测的 K线数量（最近 N 根，限制 <= 1000）
    pub limit: usize,
    pub initial_capital: f64,
    /// 单笔最大风险（账户占比，如 0.02 = 2%）
    pub risk_per_trade: f64,
    /// 风险收益比
    pub rr_ratio: f64,
    /// 止损算法
    pub stop_kind: StopKind,
    /// ATR 倍数（StopKind::Atr 时使用）
    pub atr_multiplier: f64,
    /// 手续费 bps（如 5 表示 0.05%）
    pub fee_bps: f64,
    /// 滑点 bps
    pub slippage_bps: f64,
    /// 均线算法
    pub ma_kind: MaKind,
    /// 均线周期
    pub ma_periods: Vec<usize>,
    /// 信号使用的基准均线周期
    pub base_period: usize,
    /// K线形态最小强度门槛（仅此值以上才触发）
    pub min_pattern_strength: u8,
    /// 回测模式：是否允许空头
    pub allow_short: bool,
}

impl Default for BacktestConfig {
    fn default() -> Self {
        Self {
            symbol: "BTCUSDT".into(),
            interval: "4h".into(),
            limit: 1000,
            initial_capital: 10_000.0,
            risk_per_trade: 0.02,
            rr_ratio: 2.0,
            stop_kind: StopKind::Atr,
            atr_multiplier: 1.5,
            fee_bps: 5.0,
            slippage_bps: 5.0,
            ma_kind: MaKind::Sma,
            ma_periods: vec![5, 10, 20, 60, 120],
            base_period: 60, // 原书 ma p.155：葛南维沪深基准 = 60 日季线
            min_pattern_strength: 4,
            allow_short: true,
        }
    }
}

impl BacktestConfig {
    pub fn timeframe(&self) -> Option<Timeframe> {
        Timeframe::parse(&self.interval)
    }
}

/// 一笔模拟交易
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trade {
    pub id: usize,
    pub side: Side,
    pub entry_index: usize,
    pub entry_time: i64,
    pub entry_price: f64,
    pub stop_loss: f64,
    pub take_profit: f64,
    pub qty: f64,
    pub exit_index: Option<usize>,
    pub exit_time: Option<i64>,
    pub exit_price: Option<f64>,
    pub exit_reason: Option<ExitReason>,
    /// 含手续费后的盈亏（账户货币）
    pub pnl: f64,
    /// 盈亏 R 倍数（pnl / risk_amount）
    pub r_multiple: f64,
    /// 触发原因（人类可读）
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExitReason {
    StopLoss,
    TakeProfit,
    Reverse,
    EndOfData,
}

/// 权益曲线采样点
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct EquityPoint {
    pub time: i64,
    pub equity: f64,
    pub drawdown: f64,
}

/// 每个形态的独立统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternStat {
    pub label: String,
    pub count: usize,
    pub wins: usize,
    pub losses: usize,
    pub total_r: f64,
    pub avg_r: f64,
    pub winrate: f64,
}

/// 回测绩效指标（PRD §E4）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Performance {
    pub total_return_pct: f64,
    pub annualized_return_pct: f64,
    pub max_drawdown_pct: f64,
    pub max_drawdown_duration_bars: usize,
    pub win_rate: f64,
    pub profit_factor: f64,
    pub avg_win: f64,
    pub avg_loss: f64,
    pub expectancy_r: f64,
    pub sharpe: f64,
    pub sortino: f64,
    pub calmar: f64,
    pub total_trades: usize,
    pub wins: usize,
    pub losses: usize,
    pub max_consec_wins: usize,
    pub max_consec_losses: usize,
    pub avg_hold_bars: f64,
}

/// 回测完整结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BacktestResult {
    pub config: BacktestConfig,
    pub bars: usize,
    pub start_time: i64,
    pub end_time: i64,
    pub performance: Performance,
    pub equity: Vec<EquityPoint>,
    pub trades: Vec<Trade>,
    pub pattern_stats: Vec<PatternStat>,
}
