//! 持久化升级种子（Promoted Seeds）
//!
//! # 目的
//!
//! 用户通过 Discovery 发现的冠军体系只存在于内存里——退出进程就丢了。本模块
//! 提供一个**本地 JSON 文件**来保存用户"入库"的体系，在下次启动时合并到
//! 种子列表。
//!
//! # 文件格式
//!
//! ```text
//! {cache_dir}/promoted_seeds.json
//! ```
//!
//! ```json
//! {
//!   "schema_version": 1,
//!   "seeds": [
//!     { "id": "promoted.xxx", "name": "...", "origin": "Discovered", ... },
//!     ...
//!   ]
//! }
//! ```
//!
//! # 并发
//!
//! 本实现不做文件锁：假设同一时间只有一个 HTTP 进程在写。HTTP handler 层
//! 在外部用 `Mutex` 串行化写操作（见 `system_routes::handle_promote`）。

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::definition::{SystemDefinition, SystemOrigin};

const FILE_NAME: &str = "promoted_seeds.json";
pub const PROMOTED_ID_PREFIX: &str = "promoted.";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct VaultFile {
    #[serde(default)]
    schema_version: u32,
    seeds: Vec<SystemDefinition>,
}

fn vault_path(cache_dir: &Path) -> PathBuf {
    cache_dir.join(FILE_NAME)
}

/// 读取全部已入库体系；文件不存在时返回空
pub fn load_promoted(cache_dir: &Path) -> Vec<SystemDefinition> {
    let p = vault_path(cache_dir);
    match fs::read_to_string(&p) {
        Ok(s) => match serde_json::from_str::<VaultFile>(&s) {
            Ok(f) => f.seeds,
            Err(e) => {
                log::warn!("promoted_seeds.json 解析失败，忽略：{}", e);
                Vec::new()
            }
        },
        Err(e) if e.kind() == io::ErrorKind::NotFound => Vec::new(),
        Err(e) => {
            log::warn!("读取 promoted_seeds.json 失败：{}", e);
            Vec::new()
        }
    }
}

fn save_all(cache_dir: &Path, seeds: &[SystemDefinition]) -> io::Result<()> {
    fs::create_dir_all(cache_dir)?;
    let file = VaultFile {
        schema_version: 1,
        seeds: seeds.to_vec(),
    };
    let body = serde_json::to_vec_pretty(&file)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.to_string()))?;
    let tmp = vault_path(cache_dir).with_extension("json.tmp");
    fs::write(&tmp, &body)?;
    fs::rename(tmp, vault_path(cache_dir))?;
    Ok(())
}

/// 入库一个体系。规则：
///
/// - 若输入 `id` 未以 `promoted.` 开头，强制加前缀避免与 hardcoded seed 冲突
/// - 若已有同 id，覆盖旧版本（允许更新）
/// - `origin` 强制为 `Discovered`
/// - `meta.created_at_ms` 若为 0 则填当前时间
///
/// 返回入库后的定义（含规范化后的 id）
pub fn add_promoted(
    cache_dir: &Path,
    mut def: SystemDefinition,
) -> Result<SystemDefinition, String> {
    if def.components.is_empty() {
        return Err("体系必须至少包含一个组件".into());
    }
    // 规范化 id
    if !def.id.starts_with(PROMOTED_ID_PREFIX) {
        def.id = format!("{}{}", PROMOTED_ID_PREFIX, def.id);
    }
    def.origin = SystemOrigin::Discovered;
    if def.meta.created_at_ms == 0 {
        def.meta.created_at_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
    }

    let mut seeds = load_promoted(cache_dir);
    // 覆盖同 id
    seeds.retain(|s| s.id != def.id);
    seeds.push(def.clone());
    save_all(cache_dir, &seeds).map_err(|e| format!("保存失败：{}", e))?;
    Ok(def)
}

/// 按 id 移除一个已入库体系；返回是否有条目被移除
pub fn remove_promoted(cache_dir: &Path, id: &str) -> Result<bool, String> {
    let mut seeds = load_promoted(cache_dir);
    let before = seeds.len();
    seeds.retain(|s| s.id != id);
    if seeds.len() == before {
        return Ok(false);
    }
    save_all(cache_dir, &seeds).map_err(|e| format!("保存失败：{}", e))?;
    Ok(true)
}

// ============================================================
// 测试
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::system::{registry::find_seed, SystemDefinition};

    fn tmp_dir(tag: &str) -> PathBuf {
        let base = std::env::temp_dir().join(format!(
            "aura_vault_test_{}_{}",
            tag,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        fs::create_dir_all(&base).unwrap();
        base
    }

    #[test]
    fn t_load_empty_when_no_file() {
        let dir = tmp_dir("empty");
        let seeds = load_promoted(&dir);
        assert!(seeds.is_empty());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn t_add_remove_roundtrip() {
        let dir = tmp_dir("roundtrip");
        let mut def: SystemDefinition = find_seed("seed.main_surge").unwrap();
        def.id = "my-new".into(); // 触发 id 规范化
        def.name = "我的冠军".into();

        let saved = add_promoted(&dir, def.clone()).unwrap();
        assert_eq!(saved.id, "promoted.my-new");
        assert!(matches!(saved.origin, SystemOrigin::Discovered));
        assert!(saved.meta.created_at_ms > 0);

        let loaded = load_promoted(&dir);
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, "promoted.my-new");
        assert_eq!(loaded[0].name, "我的冠军");

        let removed = remove_promoted(&dir, "promoted.my-new").unwrap();
        assert!(removed);
        assert!(load_promoted(&dir).is_empty());

        // 再次移除应返回 false
        assert!(!remove_promoted(&dir, "promoted.my-new").unwrap());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn t_add_overwrites_same_id() {
        let dir = tmp_dir("overwrite");
        let mut d1: SystemDefinition = find_seed("seed.main_surge").unwrap();
        d1.id = "dup".into();
        d1.name = "v1".into();
        add_promoted(&dir, d1).unwrap();

        let mut d2: SystemDefinition = find_seed("seed.main_surge").unwrap();
        d2.id = "dup".into();
        d2.name = "v2".into();
        add_promoted(&dir, d2).unwrap();

        let loaded = load_promoted(&dir);
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].name, "v2");
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn t_rejects_empty_components() {
        let dir = tmp_dir("empty_comp");
        let mut def: SystemDefinition = find_seed("seed.main_surge").unwrap();
        def.components.clear();
        def.id = "bad".into();
        assert!(add_promoted(&dir, def).is_err());
        fs::remove_dir_all(&dir).ok();
    }
}
