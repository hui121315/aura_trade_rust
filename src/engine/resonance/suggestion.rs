//! 建议计算器：给定共振得分 + 账户参数 + ATR，输出具体交易建议
//!
//! **重要**：本系统仅做决策辅助与回测，不做自动下单，建议包含：
//! - 推荐方向
//! - 推荐仓位（基于单笔风险）
//! - 入场、止损、止盈价格
//! - 风险金额与预计 R:R

use serde::{Deserialize, Serialize};

use super::score::{ResonanceScore, Stance};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeSuggestion {
    pub stance: Stance,
    pub stance_label: String,
    /// 建议方向：+1 做多 / -1 做空 / 0 观望
    pub direction: i8,
    /// 建议的参与度（0~1）：在共振足够强才推满仓
    pub confidence: f64,
    /// 建议单笔风险（账户占比）
    pub suggested_risk_pct: f64,
    /// 建议仓位（货币数量）
    pub suggested_position_size: f64,
    /// 建议仓位对应的金额
    pub suggested_notional: f64,
    /// 入场价
    pub entry_price: f64,
    /// 止损价
    pub stop_loss: f64,
    /// 止盈价
    pub take_profit: f64,
    /// 风险（账户货币）
    pub risk_amount: f64,
    /// 潜在收益（账户货币）
    pub reward_amount: f64,
    /// R:R 比
    pub rr_ratio: f64,
    /// 人类可读的推理说明
    pub rationale: Vec<String>,
}

/// 输入配置
pub struct SuggestionInput {
    pub account_equity: f64,
    pub current_price: f64,
    pub atr: f64,
    pub max_risk_pct: f64, // 比如 0.02（2%）
    pub rr_target: f64,    // 比如 2.0
    pub atr_stop_mult: f64, // 止损距离 = ATR × mult
}

impl Default for SuggestionInput {
    fn default() -> Self {
        Self {
            account_equity: 10_000.0,
            current_price: 0.0,
            atr: 0.0,
            max_risk_pct: 0.02,
            rr_target: 2.0,
            atr_stop_mult: 1.5,
        }
    }
}

pub fn compute_suggestion(score: &ResonanceScore, inp: &SuggestionInput) -> TradeSuggestion {
    let mut rationale: Vec<String> = Vec::new();

    // 1. 方向与参与度
    let (direction, confidence): (i8, f64) = match score.stance {
        Stance::StrongBull => (1, 1.0),
        Stance::Bull => (1, 0.7),
        Stance::WeakBull => (1, 0.35),
        Stance::Neutral => (0, 0.0),
        Stance::WeakBear => (-1, 0.35),
        Stance::Bear => (-1, 0.7),
        Stance::StrongBear => (-1, 1.0),
    };

    // 方向一致性小于 0.6 降级信心
    let confidence = if score.alignment < 0.6 { confidence * 0.5 } else { confidence };

    rationale.push(format!("共振总分 {:+.1}（{}）；维度一致性 {:.0}%",
        score.total, score.stance_label, score.alignment * 100.0));

    // 2. 仓位 = 信心 × max_risk / (stop_distance / price)
    // 先算止损距离
    let stop_distance = (inp.atr * inp.atr_stop_mult).max(inp.current_price * 0.003);
    let entry = inp.current_price;
    let stop_loss = if direction > 0 { entry - stop_distance } else if direction < 0 { entry + stop_distance } else { entry };
    let take_profit = if direction > 0 { entry + stop_distance * inp.rr_target } else if direction < 0 { entry - stop_distance * inp.rr_target } else { entry };

    // 3. 账户风险
    let effective_risk_pct = inp.max_risk_pct * confidence;
    let risk_amount = inp.account_equity * effective_risk_pct;
    let position_size = if stop_distance > 0.0 && direction != 0 {
        risk_amount / stop_distance
    } else {
        0.0
    };
    let notional = position_size * entry;
    let reward_amount = risk_amount * inp.rr_target;

    rationale.push(format!("按信心 {:.0}%，采用 {:.2}% 风险比 = {:.2} USD",
        confidence * 100.0, effective_risk_pct * 100.0, risk_amount));
    rationale.push(format!("ATR(14) × {} = {:.2} 作为止损距离",
        inp.atr_stop_mult, stop_distance));
    if direction != 0 {
        rationale.push(format!("入场 @{:.2}，止损 @{:.2}，止盈 @{:.2}（R:R = 1:{}）",
            entry, stop_loss, take_profit, inp.rr_target));
    } else {
        rationale.push("共振不足，建议观望".into());
    }

    // 关键维度解释
    for d in &score.dimensions {
        if d.score.abs() >= 5.0 {
            rationale.push(format!("{} {:+.1}", d.name, d.score));
        }
    }

    TradeSuggestion {
        stance: score.stance,
        stance_label: score.stance.label().to_string(),
        direction,
        confidence,
        suggested_risk_pct: effective_risk_pct,
        suggested_position_size: position_size,
        suggested_notional: notional,
        entry_price: entry,
        stop_loss,
        take_profit,
        risk_amount,
        reward_amount,
        rr_ratio: inp.rr_target,
        rationale,
    }
}
