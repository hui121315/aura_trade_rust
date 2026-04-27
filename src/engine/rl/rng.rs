//! 轻量 PRNG 与常见概率分布采样（零外部依赖）
//!
//! - [`Xoshiro256`]：xoshiro256** 算法（64 位状态），质量足以用于 bandit 采样
//! - [`gamma_sample`]：Marsaglia & Tsang (2000) 的 Gamma 采样法
//! - [`beta_sample`]：Beta(α, β) = Gamma(α) / (Gamma(α) + Gamma(β))
//!
//! 这些实现**不是**密码学安全的，仅用于 bandit 的随机性需求。

use std::time::{SystemTime, UNIX_EPOCH};

/// xoshiro256** PRNG
///
/// 参考 https://prng.di.unimi.it/xoshiro256starstar.c
#[derive(Debug, Clone)]
pub struct Xoshiro256 {
    state: [u64; 4],
}

impl Xoshiro256 {
    /// 按指定种子构造
    pub fn from_seed(seed: u64) -> Self {
        // 用 splitmix64 把 64 位种子扩成 256 位内部状态
        let mut s = [0u64; 4];
        let mut x = seed;
        for slot in s.iter_mut() {
            x = x.wrapping_add(0x9E3779B97F4A7C15);
            let mut z = x;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
            z ^= z >> 31;
            *slot = z;
        }
        Self { state: s }
    }

    /// 用当前时间 + 进程地址作为种子，尽可能避免跨重启完全相同
    pub fn from_entropy() -> Self {
        let ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0xDEAD_BEEF_CAFE_BABE);
        let addr = &ns as *const _ as usize as u64;
        Self::from_seed(ns ^ addr.rotate_left(17))
    }

    fn rotl(x: u64, k: u32) -> u64 {
        (x << k) | (x >> (64 - k))
    }

    /// 下一个 64 位随机数
    pub fn next_u64(&mut self) -> u64 {
        let result = Self::rotl(self.state[1].wrapping_mul(5), 7).wrapping_mul(9);
        let t = self.state[1] << 17;
        self.state[2] ^= self.state[0];
        self.state[3] ^= self.state[1];
        self.state[1] ^= self.state[2];
        self.state[0] ^= self.state[3];
        self.state[2] ^= t;
        self.state[3] = Self::rotl(self.state[3], 45);
        result
    }

    /// 均匀 [0, 1) f64
    #[inline]
    pub fn next_f64(&mut self) -> f64 {
        // 高 53 位除以 2^53
        (self.next_u64() >> 11) as f64 / ((1u64 << 53) as f64)
    }

    /// 标准正态 N(0, 1)（Box-Muller）
    pub fn next_normal(&mut self) -> f64 {
        // 避免 log(0)；ε 以上
        let u1 = self.next_f64().max(1e-300);
        let u2 = self.next_f64();
        (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
    }
}

/// Marsaglia & Tsang 方法采样 Gamma(α, 1)
///
/// - α ≥ 1：直接使用论文算法
/// - 0 < α < 1：套娃法：Gamma(α) = Gamma(α+1) × U^(1/α)
pub fn gamma_sample(alpha: f64, rng: &mut Xoshiro256) -> f64 {
    assert!(alpha > 0.0, "alpha must be > 0");

    if alpha < 1.0 {
        let g = gamma_sample(alpha + 1.0, rng);
        let u = rng.next_f64().max(1e-300);
        return g * u.powf(1.0 / alpha);
    }

    // α ≥ 1
    let d = alpha - 1.0 / 3.0;
    let c = 1.0 / (9.0 * d).sqrt();
    loop {
        // 生成 N(0,1)
        let x = rng.next_normal();
        let v_base = 1.0 + c * x;
        if v_base <= 0.0 {
            continue;
        }
        let v = v_base * v_base * v_base; // (1 + c·x)^3
        let u = rng.next_f64();
        // 快速接受
        if u < 1.0 - 0.0331 * x.powi(4) {
            return d * v;
        }
        // 精确接受
        if u.ln() < 0.5 * x * x + d * (1.0 - v + v.ln()) {
            return d * v;
        }
    }
}

/// Beta(α, β) 采样：通过两个独立 Gamma
pub fn beta_sample(alpha: f64, beta: f64, rng: &mut Xoshiro256) -> f64 {
    let x = gamma_sample(alpha, rng);
    let y = gamma_sample(beta, rng);
    let sum = x + y;
    if sum < 1e-12 {
        0.5
    } else {
        (x / sum).clamp(0.0, 1.0)
    }
}

// ============================================================
// 测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_xoshiro_deterministic_from_seed() {
        let mut a = Xoshiro256::from_seed(42);
        let mut b = Xoshiro256::from_seed(42);
        for _ in 0..1000 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn t_next_f64_in_unit_interval() {
        let mut rng = Xoshiro256::from_seed(7);
        for _ in 0..10_000 {
            let v = rng.next_f64();
            assert!((0.0..1.0).contains(&v), "{} out of [0,1)", v);
        }
    }

    #[test]
    fn t_normal_mean_and_var_converge() {
        // 100k 次 N(0,1) 样本：均值 ≈ 0，方差 ≈ 1
        let mut rng = Xoshiro256::from_seed(11);
        let n = 100_000;
        let samples: Vec<f64> = (0..n).map(|_| rng.next_normal()).collect();
        let mean = samples.iter().sum::<f64>() / n as f64;
        let var = samples.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n as f64;
        assert!(mean.abs() < 0.02, "mean = {}", mean);
        assert!((var - 1.0).abs() < 0.05, "var = {}", var);
    }

    #[test]
    fn t_gamma_mean_matches_alpha() {
        // Gamma(α, 1) 的均值 = α
        let mut rng = Xoshiro256::from_seed(23);
        for alpha in [0.5, 1.0, 2.5, 5.0, 10.0] {
            let n = 20_000;
            let mean: f64 = (0..n).map(|_| gamma_sample(alpha, &mut rng)).sum::<f64>() / n as f64;
            // 允许 5% 偏差
            let rel = (mean - alpha).abs() / alpha;
            assert!(rel < 0.05, "alpha={} mean={} rel={}", alpha, mean, rel);
        }
    }

    #[test]
    fn t_beta_in_0_1_range() {
        let mut rng = Xoshiro256::from_seed(31);
        for _ in 0..10_000 {
            let v = beta_sample(2.5, 5.0, &mut rng);
            assert!((0.0..=1.0).contains(&v), "beta out of range: {}", v);
        }
    }

    #[test]
    fn t_beta_mean_matches_alpha_over_alpha_plus_beta() {
        // Beta(α, β) 的均值 = α / (α + β)
        let mut rng = Xoshiro256::from_seed(53);
        let alpha = 3.0;
        let beta = 7.0;
        let expected = alpha / (alpha + beta); // 0.3
        let n = 30_000;
        let mean: f64 =
            (0..n).map(|_| beta_sample(alpha, beta, &mut rng)).sum::<f64>() / n as f64;
        assert!((mean - expected).abs() < 0.01, "mean = {}", mean);
    }

    #[test]
    fn t_beta_small_alpha_beta() {
        // α, β < 1 触发 "套娃" 分支
        let mut rng = Xoshiro256::from_seed(71);
        for _ in 0..1000 {
            let v = beta_sample(0.5, 0.5, &mut rng);
            assert!((0.0..=1.0).contains(&v));
        }
    }
}
