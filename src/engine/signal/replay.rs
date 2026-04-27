//! F8：历史再现验证框架（R-P1-06，Sprint 10）
//!
//! 给定一段历史 K 线，**在任意时点**"再现"当时能看到的信号，
//! 并验证该信号之后 N 根 K 线的真实表现（胜率、α、最大回撤）。
//!
//! # 使用场景
//!
//! - 学习：复盘历史某个断头铡刀实际是否跑赢市场
//! - 调参：评估改变阈值后信号质量的变化
//! - 风控：批量评估某种信号在样本外的稳定性
//!
//! # 工程实现
//!
//! [`HistoricalReplay`] 持有 K 线和各识别器的输出，提供：
//!
//! - [`HistoricalReplay::replay_at`]：在索引 i 处收集所有当时可见信号
//! - [`HistoricalReplay::evaluate_signal`]：评估单个信号后续表现
//! - [`HistoricalReplay::batch_evaluate`]：批量评估所有信号
//!
//! # 不变量
//!
//! - 严格**时间因果**：replay_at(i) 只能访问 closes[0..=i]，不能透视未来
//! - 评估函数使用 closes[i..i+horizon]，与识别完全隔离

use serde::{Deserialize, Serialize};

/// 单个信号的再现记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayRecord {
    /// 信号名
    pub name: String,
    /// 触发索引
    pub index: usize,
    /// 方向
    pub direction: i8,
    /// 触发时价格
    pub price_at_signal: f64,
    /// horizon 后价格
    pub price_after: f64,
    /// 后续原始涨跌幅（未考虑方向）
    pub raw_return: f64,
    /// 方向修正后的盈亏（direction × raw_return）
    pub directional_return: f64,
    /// 方向是否正确
    pub correct: bool,
    /// horizon 内最大不利回撤（相对触发价）
    pub max_adverse_excursion: f64,
    /// horizon 内最大有利收益
    pub max_favorable_excursion: f64,
}

impl ReplayRecord {
    pub fn is_win(&self) -> bool {
        self.correct
    }

    pub fn is_loss(&self) -> bool {
        !self.correct && self.directional_return != 0.0
    }
}

/// 回放统计汇总
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReplayStats {
    pub total: usize,
    pub wins: usize,
    pub losses: usize,
    pub win_rate: f64,
    pub avg_return: f64,
    pub max_gain: f64,
    pub max_loss: f64,
    pub avg_mae: f64,
    pub avg_mfe: f64,
    pub sharpe_ratio: f64,
}

impl ReplayStats {
    pub fn from_records(records: &[ReplayRecord]) -> Self {
        if records.is_empty() {
            return Self::default();
        }
        let total = records.len();
        let wins = records.iter().filter(|r| r.is_win()).count();
        let losses = records.iter().filter(|r| r.is_loss()).count();
        let returns: Vec<f64> = records.iter().map(|r| r.directional_return).collect();
        let avg_return = returns.iter().sum::<f64>() / total as f64;
        let max_gain = returns.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let max_loss = returns.iter().cloned().fold(f64::INFINITY, f64::min);
        let avg_mae = records.iter().map(|r| r.max_adverse_excursion).sum::<f64>() / total as f64;
        let avg_mfe = records.iter().map(|r| r.max_favorable_excursion).sum::<f64>() / total as f64;

        // 简易 Sharpe：avg / std
        let var = returns.iter().map(|r| (r - avg_return).powi(2)).sum::<f64>() / total as f64;
        let std = var.sqrt();
        let sharpe_ratio = if std > 1e-9 { avg_return / std } else { 0.0 };

        Self {
            total,
            wins,
            losses,
            win_rate: wins as f64 / total as f64,
            avg_return,
            max_gain,
            max_loss,
            avg_mae,
            avg_mfe,
            sharpe_ratio,
        }
    }
}

/// 历史再现引擎
pub struct HistoricalReplay<'a> {
    closes: &'a [f64],
    horizon: usize,
}

impl<'a> HistoricalReplay<'a> {
    pub fn new(closes: &'a [f64], horizon: usize) -> Self {
        Self { closes, horizon }
    }

    /// 评估单个信号：在 `index` 点触发，方向为 `direction`
    ///
    /// # 返回
    /// - `Some(ReplayRecord)`：评估成功
    /// - `None`：索引 + horizon 超出数据范围，无法评估
    pub fn evaluate_signal(
        &self,
        name: impl Into<String>,
        index: usize,
        direction: i8,
    ) -> Option<ReplayRecord> {
        if index + self.horizon >= self.closes.len() {
            return None;
        }
        let price_at = self.closes[index];
        if !price_at.is_finite() || price_at.abs() < 1e-9 {
            return None;
        }
        let price_after = self.closes[index + self.horizon];
        let raw = (price_after - price_at) / price_at;
        let dir_ret = (direction as f64) * raw;

        let correct = match direction {
            d if d > 0 => price_after > price_at,
            d if d < 0 => price_after < price_at,
            _ => raw.abs() < 0.003,
        };

        // MAE / MFE（方向修正）
        let mut max_adverse = 0.0f64;
        let mut max_favorable = 0.0f64;
        for i in (index + 1)..=(index + self.horizon) {
            if i >= self.closes.len() {
                break;
            }
            let p = self.closes[i];
            if !p.is_finite() {
                continue;
            }
            let pnl = (direction as f64) * (p - price_at) / price_at;
            if pnl < max_adverse {
                max_adverse = pnl;
            }
            if pnl > max_favorable {
                max_favorable = pnl;
            }
        }

        Some(ReplayRecord {
            name: name.into(),
            index,
            direction,
            price_at_signal: price_at,
            price_after,
            raw_return: raw,
            directional_return: dir_ret,
            correct,
            max_adverse_excursion: max_adverse,
            max_favorable_excursion: max_favorable,
        })
    }

    /// 批量评估（便捷 API）
    pub fn batch_evaluate(
        &self,
        signals: impl IntoIterator<Item = (String, usize, i8)>,
    ) -> Vec<ReplayRecord> {
        signals
            .into_iter()
            .filter_map(|(name, idx, dir)| self.evaluate_signal(name, idx, dir))
            .collect()
    }

    /// 市场基线回报（在 index 持有 horizon 根的涨跌幅，不考虑方向）
    pub fn market_baseline(&self, index: usize) -> Option<f64> {
        if index + self.horizon >= self.closes.len() {
            return None;
        }
        let p = self.closes[index];
        if p.abs() < 1e-9 {
            return None;
        }
        Some((self.closes[index + self.horizon] - p) / p)
    }

    /// 信号相对市场的 α
    pub fn alpha_vs_market(&self, record: &ReplayRecord) -> Option<f64> {
        let mkt = self.market_baseline(record.index)?;
        Some(record.directional_return - mkt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_evaluate_signal_correct_direction() {
        // closes[10] = 110, closes[15] = 115 → raw = (115-110)/110 ≈ 0.0454
        let closes: Vec<f64> = (0..20).map(|i| 100.0 + i as f64).collect();
        let replay = HistoricalReplay::new(&closes, 5);
        let r = replay.evaluate_signal("test", 10, 1).unwrap();
        assert!(r.correct, "110→115 向上 + direction=+1 应判为正确");
        // raw = 5/110 ≈ 0.04545
        let expected = 5.0 / 110.0;
        assert!((r.raw_return - expected).abs() < 1e-9);
        assert!((r.directional_return - expected).abs() < 1e-9);
    }

    #[test]
    fn t_evaluate_signal_wrong_direction() {
        // closes 上涨 5%，但信号 direction=-1（看空）→ 判错
        let closes: Vec<f64> = (0..20).map(|i| 100.0 + i as f64).collect();
        let replay = HistoricalReplay::new(&closes, 5);
        let r = replay.evaluate_signal("test", 10, -1).unwrap();
        assert!(!r.correct);
        assert!(r.directional_return < 0.0);
    }

    #[test]
    fn t_evaluate_out_of_range_returns_none() {
        let closes = vec![100.0; 10];
        let replay = HistoricalReplay::new(&closes, 5);
        assert!(replay.evaluate_signal("t", 8, 1).is_none());
        assert!(replay.evaluate_signal("t", 100, 1).is_none());
    }

    #[test]
    fn t_mae_and_mfe_computed() {
        // closes 先跌到 95 再涨到 110
        let closes = vec![100.0, 98.0, 95.0, 97.0, 100.0, 105.0, 110.0];
        let replay = HistoricalReplay::new(&closes, 6);
        let r = replay.evaluate_signal("test", 0, 1).unwrap();
        // 多头：MAE = min((下跌到 95) / 100) = -5%
        assert!(r.max_adverse_excursion < -0.04);
        // MFE = (110 - 100) / 100 = +10%
        assert!(r.max_favorable_excursion > 0.09);
    }

    #[test]
    fn t_stats_aggregation() {
        // 构造 3 个记录：2 胜 1 负
        let records = vec![
            ReplayRecord {
                name: "a".into(),
                index: 0,
                direction: 1,
                price_at_signal: 100.0,
                price_after: 105.0,
                raw_return: 0.05,
                directional_return: 0.05,
                correct: true,
                max_adverse_excursion: -0.01,
                max_favorable_excursion: 0.06,
            },
            ReplayRecord {
                name: "b".into(),
                index: 10,
                direction: 1,
                price_at_signal: 100.0,
                price_after: 103.0,
                raw_return: 0.03,
                directional_return: 0.03,
                correct: true,
                max_adverse_excursion: -0.02,
                max_favorable_excursion: 0.04,
            },
            ReplayRecord {
                name: "c".into(),
                index: 20,
                direction: 1,
                price_at_signal: 100.0,
                price_after: 98.0,
                raw_return: -0.02,
                directional_return: -0.02,
                correct: false,
                max_adverse_excursion: -0.03,
                max_favorable_excursion: 0.01,
            },
        ];
        let stats = ReplayStats::from_records(&records);
        assert_eq!(stats.total, 3);
        assert_eq!(stats.wins, 2);
        assert_eq!(stats.losses, 1);
        assert!((stats.win_rate - 2.0 / 3.0).abs() < 1e-9);
        // avg = (0.05 + 0.03 - 0.02) / 3 = 0.02
        assert!((stats.avg_return - 0.02).abs() < 1e-9);
        assert!((stats.max_gain - 0.05).abs() < 1e-9);
        assert!((stats.max_loss - (-0.02)).abs() < 1e-9);
    }

    #[test]
    fn t_empty_stats_default_zeros() {
        let stats = ReplayStats::from_records(&[]);
        assert_eq!(stats.total, 0);
        assert_eq!(stats.win_rate, 0.0);
    }

    #[test]
    fn t_alpha_vs_market() {
        let closes: Vec<f64> = (0..20).map(|i| 100.0 + i as f64).collect();
        let replay = HistoricalReplay::new(&closes, 5);
        // direction=1：dir_ret = market → α=0
        let r = replay.evaluate_signal("t", 10, 1).unwrap();
        let alpha = replay.alpha_vs_market(&r).unwrap();
        assert!(alpha.abs() < 1e-9);
        // direction=-1（看空）：dir_ret = -market → α = -2 × market
        let r2 = replay.evaluate_signal("t", 10, -1).unwrap();
        let alpha2 = replay.alpha_vs_market(&r2).unwrap();
        let expected = -2.0 * (5.0 / 110.0);
        assert!((alpha2 - expected).abs() < 1e-9);
    }

    #[test]
    fn t_batch_evaluate() {
        let closes: Vec<f64> = (0..30).map(|i| 100.0 + i as f64).collect();
        let replay = HistoricalReplay::new(&closes, 5);
        let signals = vec![
            ("a".to_string(), 5, 1),
            ("b".to_string(), 10, 1),
            ("c".to_string(), 15, -1),
            ("d".to_string(), 29, 1), // 超出范围，应被过滤
        ];
        let records = replay.batch_evaluate(signals);
        assert_eq!(records.len(), 3); // d 被过滤
    }

    #[test]
    fn t_is_win_is_loss_exclusive() {
        let win = ReplayRecord {
            name: "w".into(),
            index: 0,
            direction: 1,
            price_at_signal: 100.0,
            price_after: 105.0,
            raw_return: 0.05,
            directional_return: 0.05,
            correct: true,
            max_adverse_excursion: 0.0,
            max_favorable_excursion: 0.05,
        };
        assert!(win.is_win());
        assert!(!win.is_loss());
    }
}
