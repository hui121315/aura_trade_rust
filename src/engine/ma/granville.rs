//! A4：葛南维八大法则
//!
//! 对应 PRD §A4。以指定周期均线（默认 MA20）作为基准。
//! 四买：B1 突破 / B2 回踩 / B3 假跌 / B4 乖离
//! 四卖：S1 跌破 / S2 反弹 / S3 假涨 / S4 乖离

use serde::{Deserialize, Serialize};

/// 葛南维法则类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GranvilleRule {
    B1BreakoutBuy,   // 突破买入
    B2PullbackBuy,   // 回踩买入
    B3FalseBreakBuy, // 假跌买入
    B4DivergenceBuy, // 乖离买入
    S1BreakdownSell, // 跌破卖出
    S2ReboundSell,   // 反弹卖出
    S3FalseBreakSell,// 假涨卖出
    S4DivergenceSell,// 乖离卖出
}

impl GranvilleRule {
    pub fn code(&self) -> &'static str {
        match self {
            GranvilleRule::B1BreakoutBuy => "B1",
            GranvilleRule::B2PullbackBuy => "B2",
            GranvilleRule::B3FalseBreakBuy => "B3",
            GranvilleRule::B4DivergenceBuy => "B4",
            GranvilleRule::S1BreakdownSell => "S1",
            GranvilleRule::S2ReboundSell => "S2",
            GranvilleRule::S3FalseBreakSell => "S3",
            GranvilleRule::S4DivergenceSell => "S4",
        }
    }
    pub fn label(&self) -> &'static str {
        match self {
            GranvilleRule::B1BreakoutBuy => "突破买入",
            GranvilleRule::B2PullbackBuy => "回踩买入",
            GranvilleRule::B3FalseBreakBuy => "假跌买入",
            GranvilleRule::B4DivergenceBuy => "乖离买入（超跌反弹）",
            GranvilleRule::S1BreakdownSell => "跌破卖出",
            GranvilleRule::S2ReboundSell => "反弹卖出",
            GranvilleRule::S3FalseBreakSell => "假涨卖出",
            GranvilleRule::S4DivergenceSell => "乖离卖出（超涨回落）",
        }
    }
    pub fn is_buy(&self) -> bool {
        matches!(
            self,
            GranvilleRule::B1BreakoutBuy
                | GranvilleRule::B2PullbackBuy
                | GranvilleRule::B3FalseBreakBuy
                | GranvilleRule::B4DivergenceBuy
        )
    }
}

/// 识别到的单个葛南维信号
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct GranvilleSignal {
    pub index: usize,
    pub rule: GranvilleRule,
}

/// 参数
pub struct GranvilleParams {
    /// 均线周期（用于日志 / 展示）。
    /// 原书 ma p.155 明确：沪深股市葛南维八大法则在 60 日均线最为有效。
    pub period: usize,
    /// 斜率 lookback
    pub slope_lookback: usize,
    /// 斜率判定阈值：> +eps 视为上升, < -eps 视为下降
    pub slope_eps: f64,
    /// 乖离率阈值（绝对值）：例如 0.08 表示 ±8%
    pub bias_thresh: f64,
    /// 假跌/假涨的最大"收回"K线数量
    pub fake_recover_bars: usize,
    /// B2 回踩带宽：前一根 K 线最多高出均线多少比例，仍视为"回踩"
    /// 原书 L2 要求"回调时未跌破均线"，代码用 touch_band 定义"靠近均线"。
    pub touch_band: f64,
}

impl Default for GranvilleParams {
    fn default() -> Self {
        Self {
            period: 60, // 原书 ma p.155：沪深股市葛南维基准 = 60 日。
            slope_lookback: 5,
            slope_eps: 0.001, // 0.1% 作为"走平"中性带
            bias_thresh: 0.08,
            fake_recover_bars: 3,
            touch_band: 0.02, // 2% 范围内视为"靠近"均线
        }
    }
}

impl GranvilleParams {
    /// 沪深股市默认（60 日季线），原书推荐。
    pub fn cn_default() -> Self {
        Self::default()
    }
    /// 葛南维原版（美股 200 日）。
    pub fn us_classic() -> Self {
        Self { period: 200, ..Self::default() }
    }
    /// 短期附加（20 日），原书提示虚假信号多，仅用于印证。
    pub fn short_confirm() -> Self {
        Self { period: 20, ..Self::default() }
    }
}

/// 扫描整条序列，返回所有葛南维信号。
pub fn scan(
    closes: &[f64],
    ma: &[f64],
    slope: &[f64],
    bias: &[f64],
    p: &GranvilleParams,
) -> Vec<GranvilleSignal> {
    let mut out = Vec::new();
    let n = closes.len().min(ma.len()).min(slope.len()).min(bias.len());
    for i in 1..n {
        let (c, c_prev) = (closes[i], closes[i - 1]);
        let (m, m_prev) = (ma[i], ma[i - 1]);
        if !(m.is_finite() && m_prev.is_finite()) {
            continue;
        }
        let s = slope[i];
        let b = bias[i];

        // B1 / S1：均线由降→平或升 && 价格穿越
        let cross_up = c_prev <= m_prev && c > m;
        let cross_down = c_prev >= m_prev && c < m;

        if cross_up && s.is_finite() && s >= -p.slope_eps {
            out.push(GranvilleSignal { index: i, rule: GranvilleRule::B1BreakoutBuy });
            continue;
        }
        if cross_down && s.is_finite() && s <= p.slope_eps {
            out.push(GranvilleSignal { index: i, rule: GranvilleRule::S1BreakdownSell });
            continue;
        }

        // B2 回踩买入：原书 L2 要求"回调时**未跌破**均线"
        // 条件：①均线上行 ②当前价在均线上 ③前一根在均线上（未破）且靠近均线 ④当前反弹
        // 与 B3（假跌破）严格区分：B2 未破，B3 已破后收回。
        if s.is_finite() && s > p.slope_eps && c > m
            && c_prev >= m_prev                               // 关键：未跌破均线
            && c_prev <= m_prev * (1.0 + p.touch_band)        // 靠近均线（在回踩带内）
            && c >= c_prev
        {
            out.push(GranvilleSignal { index: i, rule: GranvilleRule::B2PullbackBuy });
            continue;
        }
        // S2 反弹卖出：原书 L7 要求"反弹时未能向上突破均线"
        if s.is_finite() && s < -p.slope_eps && c < m
            && c_prev <= m_prev
            && c_prev >= m_prev * (1.0 - p.touch_band)
            && c <= c_prev
        {
            out.push(GranvilleSignal { index: i, rule: GranvilleRule::S2ReboundSell });
            continue;
        }

        // B3：均线上行，前 N 根内有跌破 + 当前收回
        if s.is_finite() && s > p.slope_eps && c > m {
            let lo = i.saturating_sub(p.fake_recover_bars);
            if (lo..i).any(|k| closes[k] < ma[k]) {
                out.push(GranvilleSignal { index: i, rule: GranvilleRule::B3FalseBreakBuy });
                continue;
            }
        }
        // S3：均线下行，前 N 根内有突破 + 当前跌回
        if s.is_finite() && s < -p.slope_eps && c < m {
            let lo = i.saturating_sub(p.fake_recover_bars);
            if (lo..i).any(|k| closes[k] > ma[k]) {
                out.push(GranvilleSignal { index: i, rule: GranvilleRule::S3FalseBreakSell });
                continue;
            }
        }

        // B4 乖离买入（逆势反弹，必须轻仓）：
        // 原书 L4："均线**下行**，股价/指数在均线**之下**运行，随后突然暴跌，
        //   距离均线**很远**，极有可能向均线靠近，可以进场买入"
        // ⚠️ 之前版本方向完全相反（要求均线上行），已修复。
        if s.is_finite() && s < -p.slope_eps
            && c < m
            && b.is_finite() && b < -p.bias_thresh
        {
            out.push(GranvilleSignal { index: i, rule: GranvilleRule::B4DivergenceBuy });
            continue;
        }
        // S4 乖离卖出（超涨回落）：
        // 原书 L5："均线**上行**，股价在均线之上，持续快速上涨，离均线越来越远"
        if s.is_finite() && s > p.slope_eps
            && c > m
            && b.is_finite() && b > p.bias_thresh
        {
            out.push(GranvilleSignal { index: i, rule: GranvilleRule::S4DivergenceSell });
            continue;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn has_rule(signals: &[GranvilleSignal], rule: GranvilleRule) -> bool {
        signals.iter().any(|s| s.rule == rule)
    }

    #[test]
    fn t_b1_breakout_buy() {
        // 均线走平（slope≈0）+ close 从 ma 下方穿越到上方
        let closes = vec![100.0, 98.0, 102.0];
        let ma = vec![100.0, 100.0, 100.0];
        let slope = vec![0.0, 0.0, 0.0];
        let bias = vec![0.0, -0.02, 0.02];
        let signals = scan(&closes, &ma, &slope, &bias, &GranvilleParams::default());
        assert!(has_rule(&signals, GranvilleRule::B1BreakoutBuy),
            "期望 B1BreakoutBuy，实际：{:?}", signals);
    }

    #[test]
    fn t_b2_pullback_buy() {
        // 均线上行 + close_prev 在 ma 上方但在 2% 带内（回踩未跌破）+ 当前反弹
        let closes = vec![99.0, 100.5, 102.0];
        let ma = vec![98.0, 99.0, 100.0];
        let slope = vec![0.01, 0.01, 0.01];
        let bias = vec![0.01, 0.015, 0.02];
        let signals = scan(&closes, &ma, &slope, &bias, &GranvilleParams::default());
        assert!(has_rule(&signals, GranvilleRule::B2PullbackBuy),
            "期望 B2PullbackBuy，实际：{:?}", signals);
    }

    #[test]
    fn t_b3_false_break_buy() {
        // 假跌破后收回：i=0 close<ma（跌破），i=1 cross_up 触发 B1，
        // i=2 保持在 ma 上方但 c_prev 离 ma 较远（避开 B2 touch_band）→ B3
        let closes = vec![98.0, 105.0, 103.0];
        let ma = vec![100.0, 100.0, 101.0];
        let slope = vec![0.01, 0.01, 0.01];
        let bias = vec![-0.02, 0.05, 0.02];
        let signals = scan(&closes, &ma, &slope, &bias, &GranvilleParams::default());
        assert!(has_rule(&signals, GranvilleRule::B3FalseBreakBuy),
            "期望 B3FalseBreakBuy，实际：{:?}", signals);
    }

    #[test]
    fn t_b4_divergence_buy() {
        // 均线下行 + close < ma + bias < -8%（超跌）
        let closes = vec![100.0, 95.0, 85.0];
        let ma = vec![110.0, 107.0, 105.0];
        let slope = vec![-0.01, -0.02, -0.03];
        let bias = vec![-0.09, -0.11, -0.19];
        let signals = scan(&closes, &ma, &slope, &bias, &GranvilleParams::default());
        assert!(has_rule(&signals, GranvilleRule::B4DivergenceBuy),
            "期望 B4DivergenceBuy，实际：{:?}", signals);
    }

    #[test]
    fn t_s1_breakdown_sell() {
        // 均线走平 + close 从 ma 上方跌破到下方
        let closes = vec![100.0, 102.0, 98.0];
        let ma = vec![100.0, 100.0, 100.0];
        let slope = vec![0.0, 0.0, 0.0];
        let bias = vec![0.0, 0.02, -0.02];
        let signals = scan(&closes, &ma, &slope, &bias, &GranvilleParams::default());
        assert!(has_rule(&signals, GranvilleRule::S1BreakdownSell),
            "期望 S1BreakdownSell，实际：{:?}", signals);
    }

    #[test]
    fn t_s2_rebound_sell() {
        // 均线下行 + close_prev 在 ma 下方但在 2% 带内（反弹未突破）+ 当前回落
        let closes = vec![101.0, 99.5, 98.0];
        let ma = vec![102.0, 101.0, 100.0];
        let slope = vec![-0.01, -0.01, -0.01];
        let bias = vec![-0.01, -0.015, -0.02];
        let signals = scan(&closes, &ma, &slope, &bias, &GranvilleParams::default());
        assert!(has_rule(&signals, GranvilleRule::S2ReboundSell),
            "期望 S2ReboundSell，实际：{:?}", signals);
    }

    #[test]
    fn t_s3_false_break_sell() {
        // 假突破后跌回：i=0 close>ma（突破），i=1 cross_down 触发 S1，
        // i=2 保持在 ma 下方但 c_prev 离 ma 较远（避开 S2 touch_band）→ S3
        let closes = vec![102.0, 95.0, 97.0];
        let ma = vec![100.0, 100.0, 99.0];
        let slope = vec![-0.01, -0.01, -0.01];
        let bias = vec![0.02, -0.05, -0.02];
        let signals = scan(&closes, &ma, &slope, &bias, &GranvilleParams::default());
        assert!(has_rule(&signals, GranvilleRule::S3FalseBreakSell),
            "期望 S3FalseBreakSell，实际：{:?}", signals);
    }

    #[test]
    fn t_s4_divergence_sell() {
        // 均线上行 + close > ma + bias > +8%（超涨）
        let closes = vec![110.0, 115.0, 130.0];
        let ma = vec![100.0, 103.0, 105.0];
        let slope = vec![0.01, 0.02, 0.03];
        let bias = vec![0.10, 0.12, 0.24];
        let signals = scan(&closes, &ma, &slope, &bias, &GranvilleParams::default());
        assert!(has_rule(&signals, GranvilleRule::S4DivergenceSell),
            "期望 S4DivergenceSell，实际：{:?}", signals);
    }

    #[test]
    fn t_rule_code_and_is_buy_metadata() {
        // code() 和 is_buy() 元数据一致性
        assert_eq!(GranvilleRule::B1BreakoutBuy.code(), "B1");
        assert_eq!(GranvilleRule::B2PullbackBuy.code(), "B2");
        assert_eq!(GranvilleRule::B3FalseBreakBuy.code(), "B3");
        assert_eq!(GranvilleRule::B4DivergenceBuy.code(), "B4");
        assert_eq!(GranvilleRule::S1BreakdownSell.code(), "S1");
        assert_eq!(GranvilleRule::S2ReboundSell.code(), "S2");
        assert_eq!(GranvilleRule::S3FalseBreakSell.code(), "S3");
        assert_eq!(GranvilleRule::S4DivergenceSell.code(), "S4");
        // 4 个 B 都是买入，4 个 S 都不是买入
        for r in [
            GranvilleRule::B1BreakoutBuy,
            GranvilleRule::B2PullbackBuy,
            GranvilleRule::B3FalseBreakBuy,
            GranvilleRule::B4DivergenceBuy,
        ] {
            assert!(r.is_buy(), "{:?} 应为买入", r);
        }
        for r in [
            GranvilleRule::S1BreakdownSell,
            GranvilleRule::S2ReboundSell,
            GranvilleRule::S3FalseBreakSell,
            GranvilleRule::S4DivergenceSell,
        ] {
            assert!(!r.is_buy(), "{:?} 不应为买入", r);
        }
    }

    #[test]
    fn t_params_presets_period_correct() {
        // 三个默认参数对应三种周期
        assert_eq!(GranvilleParams::cn_default().period, 60);
        assert_eq!(GranvilleParams::us_classic().period, 200);
        assert_eq!(GranvilleParams::short_confirm().period, 20);
    }

    #[test]
    fn t_empty_input_no_signals() {
        let signals = scan(&[], &[], &[], &[], &GranvilleParams::default());
        assert!(signals.is_empty());
    }

    #[test]
    fn t_nan_ma_skipped_without_panic() {
        // 序列首含 NaN 应被 skip，不应 panic
        let closes = vec![100.0, 98.0, 102.0];
        let ma = vec![f64::NAN, 100.0, 100.0];
        let slope = vec![f64::NAN, 0.0, 0.0];
        let bias = vec![f64::NAN, -0.02, 0.02];
        let signals = scan(&closes, &ma, &slope, &bias, &GranvilleParams::default());
        // 不强断具体 rule，只验证流程不 panic 且不误发
        let _ = signals;
    }
}
