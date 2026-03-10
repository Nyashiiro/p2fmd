//! 扫描模块：封装 WalkDir 和 Rayon，负责高性能目录遍历与早期剪枝

use crate::config::AppConfig;
use crate::matcher::{evaluate_rules, Action};
use crate::utils::{log_message, has_hidden_component};
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use walkdir::{DirEntry, WalkDir};

/// 检查目录项是否为隐藏项
fn is_hidden(entry: &DirEntry) -> bool {
    entry.file_name()
        .to_str()
        .map(|s| s.starts_with('.'))
        .unwrap_or(false)
}

/// 收集文件（支持并行或串行处理）
/// 
/// 核心优化：
/// 1. 早期剪枝：在 WalkDir 阶段使用 filter_entry 拦截隐藏目录
/// 2. 短路匹配：逆序遍历规则，匹配到高优先级规则立即返回
pub fn collect_files(
    exec_root: &Path,
    config: &AppConfig,
    no_parallel: bool,
) -> anyhow::Result<Vec<PathBuf>> {
    let unix_hide = config.unix_hide;
    
    // 1. 早期剪枝目录遍历（避免进入 node_modules 或 .git 等隐藏深坑）
    let candidate_files: Vec<PathBuf> = WalkDir::new(exec_root)
        .follow_links(false)  // 禁用符号链接跟随，避免循环
        .into_iter()
        .filter_entry(move |e| {
            // 如果是隐藏目录，直接截断整棵子树
            if unix_hide && is_hidden(e) && e.file_type().is_dir() {
                return false;
            }
            true
        })
        .filter_map(|entry_result| {
            match entry_result {
                Ok(entry) => {
                    // 只保留文件
                    if entry.file_type().is_file() {
                        Some(entry.into_path())
                    } else {
                        None
                    }
                }
                Err(e) => {
                    log_message("WARN", format!("遍历目录时出错: {}", e));
                    None
                }
            }
        })
        .collect();

    if candidate_files.is_empty() {
        return Ok(Vec::new());
    }

    log_message("INFO", format!("发现 {} 个候选文件，开始过滤...", candidate_files.len()));

    // 2. 并行或串行进行规则过滤
    let filter_logic = |path: &PathBuf| -> bool {
        // 计算相对路径
        let rel_path = match path.strip_prefix(exec_root) {
            Ok(p) => p,
            Err(_) => {
                log_message("WARN", format!("无法计算相对路径: {:?}", path));
                return false;
            }
        };
        
        let filename = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");
        
        let dirpath = rel_path.parent().unwrap_or_else(|| Path::new(""));

        // Unix 隐藏文件检查
        if unix_hide && has_hidden_component(rel_path) {
            // 检查是否有规则强制包含
            if let Some(action) = evaluate_rules(&config.rules, rel_path, filename, dirpath) {
                return action == Action::Include;
            }
            return false;
        }

        // 基础扩展名检查
        let ext_ok = if config.allowed_exts.is_empty() {
            true
        } else {
            path.extension()
                .and_then(|s| s.to_str())
                .map(|ext| config.allowed_exts.contains(&ext.to_lowercase()))
                .unwrap_or(false)
        };

        // 逆序匹配引擎（优先级最高的最后定义）
        if let Some(action) = evaluate_rules(&config.rules, rel_path, filename, dirpath) {
            return action == Action::Include;
        }

        // 默认行为：如果没有规则匹配，根据扩展名决定
        ext_ok
    };

    let mut matched: Vec<PathBuf> = if no_parallel {
        candidate_files.into_iter()
            .filter(filter_logic)
            .collect()
    } else {
        candidate_files.into_par_iter()
            .filter(filter_logic)
            .collect()
    };

    // 排序确保稳定输出
    matched.sort_by(|a, b| {
        let a_rel = a.strip_prefix(exec_root).unwrap_or(a);
        let b_rel = b.strip_prefix(exec_root).unwrap_or(b);
        a_rel.cmp(b_rel)
    });

    // 去重
    matched.dedup();

    Ok(matched)
}

/// 串行版本（保留用于兼容性）
pub fn collect_files_serial(
    exec_root: &Path,
    config: &AppConfig,
) -> anyhow::Result<Vec<PathBuf>> {
    collect_files(exec_root, config, true)
}

/// 并行版本（保留用于兼容性）
pub fn collect_files_parallel(
    exec_root: &Path,
    config: &AppConfig,
) -> anyhow::Result<Vec<PathBuf>> {
    collect_files(exec_root, config, false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn create_test_config() -> AppConfig {
        AppConfig {
            unix_hide: true,
            allowed_exts: HashSet::from(["rs".to_string()]),
            rules: Vec::new(),
        }
    }

    #[test]
    fn test_is_hidden() {
        let tmp_dir = std::env::temp_dir();
        // 注意：这里只是测试函数逻辑，不涉及实际文件系统
        assert!(is_hidden(&DirEntry::from_path(
            &tmp_dir.join(".hidden"),
            walkdir::DirEntryInner::new(&tmp_dir.join(".hidden")).unwrap()
        ).unwrap()));
    }
}
