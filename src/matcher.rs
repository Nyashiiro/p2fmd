//! 匹配引擎：负责 Glob/正则表达式的预编译，以及核心的"逆序短路匹配"逻辑

use globset::{Glob, GlobMatcher};
use regex::Regex;
use serde::Deserialize;
use std::path::Path;

/// 匹配动作
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    /// 包含文件
    Include,
    /// 隐藏文件
    Hide,
}

/// 预编译后的条件
pub struct CompiledCondition {
    /// 文件名匹配器（OR 关系）
    pub name_matchers: Vec<GlobMatcher>,
    /// 路径匹配器（OR 关系）
    pub path_matchers: Vec<GlobMatcher>,
    /// 正则匹配器（OR 关系）
    pub regex_matchers: Vec<Regex>,
}

/// 编译后的规则
pub struct CompiledRule {
    /// 匹配动作
    pub action: Action,
    /// 预编译条件
    pub condition: CompiledCondition,
}

impl CompiledRule {
    /// 短路匹配核心：只要有一个维度的条件不满足，立刻返回 false
    /// 所有维度之间是 AND 关系
    pub fn matches(&self, rel_path: &Path, filename: &str, dirpath: &Path) -> bool {
        // 1. 匹配 name (OR 关系，任一满足即可)
        if !self.condition.name_matchers.is_empty() {
            let matched = self.condition.name_matchers.iter().any(|g| g.is_match(filename));
            if !matched {
                return false;
            }
        }

        // 2. 匹配 path (OR 关系)
        if !self.condition.path_matchers.is_empty() {
            let dir_str = dirpath.to_string_lossy();
            let matched = self.condition.path_matchers.iter().any(|g| {
                g.is_match(&*dir_str) || g.is_match(dirpath)
            });
            if !matched {
                return false;
            }
        }

        // 3. 匹配 regex (OR 关系，对完整相对路径)
        if !self.condition.regex_matchers.is_empty() {
            let path_str = rel_path.to_string_lossy();
            let matched = self.condition.regex_matchers.iter().any(|r| r.is_match(&path_str));
            if !matched {
                return false;
            }
        }

        // 全部条件都通过 (AND 关系)
        true
    }
}

/// 编译 Glob 模式
pub fn compile_glob(pattern: &str) -> anyhow::Result<GlobMatcher> {
    // 处理空模式
    if pattern.is_empty() {
        return Err(anyhow::anyhow!("空 glob 模式"));
    }
    
    // 自动添加 **/ 前缀以支持递归匹配
    let adjusted_pattern = if !pattern.contains('/') {
        format!("**/{}", pattern)
    } else {
        pattern.to_string()
    };
    
    let glob = Glob::new(&adjusted_pattern)
        .map_err(|e| anyhow::anyhow!("无效的 glob 模式 '{}': {}", pattern, e))?;
    
    Ok(glob.compile_matcher())
}

/// 编译正则表达式
pub fn compile_regex(pattern: &str) -> anyhow::Result<Regex> {
    Regex::new(pattern)
        .map_err(|e| anyhow::anyhow!("无效的正则表达式 '{}': {}", pattern, e))
}

/// 规则引擎：逆序遍历规则，短路返回
/// 
/// 优化原理：后定义的规则优先级更高，逆序遍历可以在匹配到高优先级规则时立即返回，
/// 避免不必要的正则和通配符计算
pub fn evaluate_rules(
    rules: &[CompiledRule],
    rel_path: &Path,
    filename: &str,
    dirpath: &Path,
) -> Option<Action> {
    // 逆序遍历实现短路优化
    for rule in rules.iter().rev() {
        if rule.matches(rel_path, filename, dirpath) {
            return Some(rule.action.clone());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_compile_glob() {
        let matcher = compile_glob("*.rs").unwrap();
        assert!(matcher.is_match("main.rs"));
        assert!(matcher.is_match("lib.rs"));
        assert!(!matcher.is_match("main.txt"));
    }

    #[test]
    fn test_compile_regex() {
        let regex = compile_regex(r".*\.rs$").unwrap();
        assert!(regex.is_match("src/main.rs"));
        assert!(!regex.is_match("src/main.txt"));
    }

    #[test]
    fn test_rule_matches() {
        let rule = CompiledRule {
            action: Action::Include,
            condition: CompiledCondition {
                name_matchers: vec![compile_glob("*.rs").unwrap()],
                path_matchers: vec![],
                regex_matchers: vec![],
            },
        };
        
        assert!(rule.matches(Path::new("src/main.rs"), "main.rs", Path::new("src")));
        assert!(!rule.matches(Path::new("src/main.txt"), "main.txt", Path::new("src")));
    }

    #[test]
    fn test_evaluate_rules_short_circuit() {
        let rules = vec![
            CompiledRule {
                action: Action::Hide,
                condition: CompiledCondition {
                    name_matchers: vec![compile_glob("*.log").unwrap()],
                    path_matchers: vec![],
                    regex_matchers: vec![],
                },
            },
            CompiledRule {
                action: Action::Include,
                condition: CompiledCondition {
                    name_matchers: vec![compile_glob("debug.log").unwrap()],
                    path_matchers: vec![],
                    regex_matchers: vec![],
                },
            },
        ];
        
        // 逆序匹配：debug.log 应该先匹配到 Include 规则
        let result = evaluate_rules(&rules, Path::new("debug.log"), "debug.log", Path::new(""));
        assert_eq!(result, Some(Action::Include));
    }
}
