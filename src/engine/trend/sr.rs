//! B4：支撑 / 压力位聚类
//!
//! 将摆动高低点按价格邻近度聚类，得到关键水平位；
//! 每个水平位的 "strength" = 触及次数 × 时间跨度。
//!
//! # 角色翻转（E30，trend p.167 / p.170 原书铁证）
//!
//! > "**支撑一旦被击穿，即成为压力；压力一旦被突破，即成为支撑**"
//!
//! 本模块通过 [`SrLevel::role_history`] 记录角色变化，并提供
//! [`SrLevel::current_role_after_bar`] 方法判断指定 K 线时刻的角色。

use serde::{Deserialize, Serialize};

use super::swing::{SwingKind, SwingPoint};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SrKind {
    Support,
    Resistance,
    Both,
}

/// 角色翻转事件（E30）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoleFlip {
    /// 支撑 → 压力（价格跌破后反弹至此遇阻）
    SupportToResistance,
    /// 压力 → 支撑（价格突破后回踩至此获支撑）
    ResistanceToSupport,
}

/// 角色翻转历史记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleHistory {
    pub at_index: usize,
    pub flip: RoleFlip,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SrLevel {
    pub price: f64,
    pub kind: SrKind,
    pub touches: usize,
    pub last_touch_index: usize,
    pub first_touch_index: usize,
    /// 综合强度分（0-1）
    pub strength: f64,
    /// 角色翻转历史（E30：支撑↔压力互换）
    #[serde(default)]
    pub role_history: Vec<RoleHistory>,
}

impl SrLevel {
    /// 在指定 K 线索引处的当前角色（考虑历史翻转）
    ///
    /// - 返回 [`SrKind::Support`] / [`SrKind::Resistance`] —— 最新的明确角色
    /// - 返回 [`SrKind::Both`] —— 没有翻转记录，保持初始角色
    ///
    /// 原书 trend p.167 / p.170：支撑被击穿变压力，压力被突破变支撑。
    pub fn current_role_after_bar(&self, bar_index: usize) -> SrKind {
        // 找到最近一次 ≤ bar_index 的翻转
        let last_flip = self
            .role_history
            .iter()
            .filter(|r| r.at_index <= bar_index)
            .max_by_key(|r| r.at_index);
        match last_flip {
            Some(r) => match r.flip {
                RoleFlip::SupportToResistance => SrKind::Resistance,
                RoleFlip::ResistanceToSupport => SrKind::Support,
            },
            None => self.kind,
        }
    }

    /// 检测并记录角色翻转（E30 核心逻辑）
    ///
    /// # 参数
    /// - `closes`：收盘价序列
    /// - `from_index`：从哪根 K 线开始扫描
    /// - `break_tolerance_pct`：有效击穿阈值（原书 3%）
    ///
    /// # 返回
    /// 新发现的翻转事件数
    pub fn detect_role_flips(
        &mut self,
        closes: &[f64],
        from_index: usize,
        break_tolerance_pct: f64,
    ) -> usize {
        let mut new_count = 0;
        let mut current_role = self.kind;
        // 应用已有翻转，确定当前状态
        if let Some(last) = self.role_history.iter().max_by_key(|r| r.at_index) {
            current_role = match last.flip {
                RoleFlip::SupportToResistance => SrKind::Resistance,
                RoleFlip::ResistanceToSupport => SrKind::Support,
            };
        }

        let start = from_index.max(self.last_touch_index);
        for (i, &close) in closes.iter().enumerate().skip(start) {
            let diff_pct = (close - self.price) / self.price.abs().max(1e-9);
            match current_role {
                SrKind::Support | SrKind::Both => {
                    // 有效跌破 → 翻转为压力
                    if diff_pct < -break_tolerance_pct {
                        self.role_history.push(RoleHistory {
                            at_index: i,
                            flip: RoleFlip::SupportToResistance,
                        });
                        current_role = SrKind::Resistance;
                        new_count += 1;
                    }
                }
                SrKind::Resistance => {
                    // 有效突破 → 翻转为支撑
                    if diff_pct > break_tolerance_pct {
                        self.role_history.push(RoleHistory {
                            at_index: i,
                            flip: RoleFlip::ResistanceToSupport,
                        });
                        current_role = SrKind::Support;
                        new_count += 1;
                    }
                }
            }
        }
        new_count
    }
}

pub fn cluster_levels(swings: &[SwingPoint], tolerance_pct: f64, last_bar_index: usize) -> Vec<SrLevel> {
    if swings.is_empty() {
        return vec![];
    }
    // 先分类
    let mut raw: Vec<(f64, bool, usize)> = swings
        .iter()
        .map(|s| (s.price, matches!(s.kind, SwingKind::High), s.index))
        .collect();
    raw.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    let mut clusters: Vec<Vec<(f64, bool, usize)>> = Vec::new();
    for pt in raw {
        if let Some(last) = clusters.last_mut() {
            let centroid = last.iter().map(|x| x.0).sum::<f64>() / last.len() as f64;
            if (pt.0 - centroid).abs() / centroid.abs().max(1e-9) <= tolerance_pct {
                last.push(pt);
                continue;
            }
        }
        clusters.push(vec![pt]);
    }

    let max_touch = clusters.iter().map(|c| c.len()).max().unwrap_or(1) as f64;
    let mut out: Vec<SrLevel> = clusters
        .into_iter()
        .filter(|c| c.len() >= 2) // 至少 2 次触及才算关键位
        .map(|c| {
            let price = c.iter().map(|x| x.0).sum::<f64>() / c.len() as f64;
            let highs = c.iter().filter(|x| x.1).count();
            let lows = c.len() - highs;
            let kind = if highs > 0 && lows > 0 {
                SrKind::Both
            } else if highs > 0 {
                SrKind::Resistance
            } else {
                SrKind::Support
            };
            let first = c.iter().map(|x| x.2).min().unwrap_or(0);
            let last = c.iter().map(|x| x.2).max().unwrap_or(0);
            let age_factor = ((last_bar_index - last) as f64).recip().max(0.1).min(1.0);
            let touches = c.len();
            let strength = (touches as f64 / max_touch) * 0.7 + age_factor * 0.3;
            SrLevel {
                price,
                kind,
                touches,
                last_touch_index: last,
                first_touch_index: first,
                strength,
                role_history: Vec::new(),
            }
        })
        .collect();

    // 按强度降序
    out.sort_by(|a, b| b.strength.partial_cmp(&a.strength).unwrap_or(std::cmp::Ordering::Equal));
    // 限制最多 10 条
    out.truncate(10);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_level(kind: SrKind, price: f64) -> SrLevel {
        SrLevel {
            price,
            kind,
            touches: 3,
            last_touch_index: 10,
            first_touch_index: 0,
            strength: 0.5,
            role_history: Vec::new(),
        }
    }

    #[test]
    fn t_support_breakdown_becomes_resistance() {
        // E30：支撑被有效跌破（≥3%）→ 变压力
        let mut level = make_level(SrKind::Support, 100.0);
        // closes[15] = 95.0 跌破 5%，应触发翻转
        let closes: Vec<f64> = (0..20)
            .map(|i| if i == 15 { 95.0 } else { 100.0 })
            .collect();
        let flips = level.detect_role_flips(&closes, 11, 0.03);
        assert_eq!(flips, 1);
        assert_eq!(level.role_history.len(), 1);
        assert_eq!(level.role_history[0].flip, RoleFlip::SupportToResistance);
        assert_eq!(level.current_role_after_bar(15), SrKind::Resistance);
        assert_eq!(level.current_role_after_bar(20), SrKind::Resistance);
    }

    #[test]
    fn t_resistance_breakout_becomes_support() {
        // E30：压力被有效突破（≥3%）→ 变支撑
        let mut level = make_level(SrKind::Resistance, 100.0);
        let closes: Vec<f64> = (0..20)
            .map(|i| if i == 15 { 105.0 } else { 100.0 })
            .collect();
        let flips = level.detect_role_flips(&closes, 11, 0.03);
        assert_eq!(flips, 1);
        assert_eq!(level.role_history[0].flip, RoleFlip::ResistanceToSupport);
        assert_eq!(level.current_role_after_bar(15), SrKind::Support);
    }

    #[test]
    fn t_insufficient_break_no_flip() {
        // 未达 3% 阈值 → 不翻转（原书 p.203 铁证）
        let mut level = make_level(SrKind::Support, 100.0);
        let closes: Vec<f64> = (0..20)
            .map(|i| if i == 15 { 98.0 } else { 100.0 })
            .collect();
        let flips = level.detect_role_flips(&closes, 11, 0.03);
        assert_eq!(flips, 0);
        assert_eq!(level.role_history.len(), 0);
        assert_eq!(level.current_role_after_bar(15), SrKind::Support);
    }

    #[test]
    fn t_double_flip() {
        // 先跌破变压力，后再突破变支撑
        let mut level = make_level(SrKind::Support, 100.0);
        let mut closes: Vec<f64> = vec![100.0; 30];
        closes[15] = 95.0; // 先跌破
        closes[25] = 105.0; // 后反弹突破
        let flips = level.detect_role_flips(&closes, 11, 0.03);
        assert_eq!(flips, 2);
        assert_eq!(level.role_history[0].flip, RoleFlip::SupportToResistance);
        assert_eq!(level.role_history[1].flip, RoleFlip::ResistanceToSupport);
        assert_eq!(level.current_role_after_bar(15), SrKind::Resistance);
        assert_eq!(level.current_role_after_bar(25), SrKind::Support);
    }

    #[test]
    fn t_role_before_flip_preserves_kind() {
        // 在翻转之前查询，仍返回原始 kind
        let mut level = make_level(SrKind::Support, 100.0);
        let closes: Vec<f64> = (0..20)
            .map(|i| if i == 15 { 95.0 } else { 100.0 })
            .collect();
        level.detect_role_flips(&closes, 11, 0.03);
        // 翻转点在 15，查询 14 应返回原 kind
        assert_eq!(level.current_role_after_bar(14), SrKind::Support);
        assert_eq!(level.current_role_after_bar(15), SrKind::Resistance);
    }

    #[test]
    fn t_cluster_levels_has_empty_role_history() {
        // 新创建的 SrLevel 的 role_history 应为空
        let swings = vec![
            SwingPoint {
                index: 0,
                time: 0,
                price: 100.0,
                kind: SwingKind::Low,
            },
            SwingPoint {
                index: 10,
                time: 10_000,
                price: 100.5,
                kind: SwingKind::Low,
            },
            SwingPoint {
                index: 20,
                time: 20_000,
                price: 100.2,
                kind: SwingKind::Low,
            },
        ];
        let levels = cluster_levels(&swings, 0.01, 25);
        assert!(!levels.is_empty());
        for l in &levels {
            assert!(l.role_history.is_empty());
        }
    }
}
