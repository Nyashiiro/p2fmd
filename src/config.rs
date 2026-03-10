//! 配置模块：定义 Serde 结构体，处理 TOML 文件的反序列化与默认值

use crate::matcher::{Action, CompiledCondition, CompiledRule, compile_glob, compile_regex};
use crate::utils::log_message;
use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashSet;
use std::path::Path;

/// 顶层配置结构
#[derive(Debug, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub rule: RuleConfig,
}

/// 规则配置
#[derive(Debug, Deserialize, Default)]
pub struct RuleConfig {
    #[serde(default)]
    pub basic: BasicConfig,
    #[serde(default)]
    pub attached: Vec<AttachedRule>,
}

/// 基础配置
#[derive(Debug, Deserialize)]
pub struct BasicConfig {
    #[serde(default = "default_unix_hide")]
    pub unix_hide: bool,
    #[serde(default)]
    pub extension: Vec<String>,
}

impl Default for BasicConfig {
    fn default() -> Self {
        Self {
            unix_hide: default_unix_hide(),
            extension: Vec::new(),
        }
    }
}

fn default_unix_hide() -> bool {
    true
}

/// 附加规则
#[derive(Debug, Deserialize)]
pub struct AttachedRule {
    #[serde(rename = "Action")]
    pub action: Action,
    #[serde(flatten)]
    pub conditions: ConditionUnion,
}

/// 条件联合类型（支持多种匹配方式）
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum ConditionUnion {
    /// 单一条件
    Single {
        #[serde(skip_serializing_if = "Option::is_none")]
        name: Option<PatternValue>,
        #[serde(skip_serializing_if = "Option::is_none")]
        path: Option<PatternValue>,
        #[serde(skip_serializing_if = "Option::is_none")]
        regex: Option<PatternValue>,
    },
}

/// 模式值类型（支持字符串、数组、表）
#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum PatternValue {
    /// 单个模式字符串
    Single(String),
    /// 多个模式（OR 关系）
    Array(Vec<String>),
    /// 多个模式（AND 关系）
    Map(std::collections::HashMap<String, String>),
}

/// 应用程序配置（已编译版本）
pub struct AppConfig {
    /// 是否隐藏 Unix 隐藏文件
    pub unix_hide: bool,
    /// 允许的文件扩展名集合
    pub allowed_exts: HashSet<String>,
    /// 编译后的规则列表
    pub rules: Vec<CompiledRule>,
}

/// 加载配置文件
pub fn load_config(config_path: Option<&Path>) -> Result<AppConfig> {
    if config_path.is_none() {
        log_message("WARN", "未找到配置文件，使用默认配置（unix_hide=true, 无扩展名限制）");
        return Ok(AppConfig {
            unix_hide: true,
            allowed_exts: HashSet::new(),
            rules: Vec::new(),
        });
    }

    let path = config_path.unwrap();
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("无法读取配置文件: {:?}", path))?;
    
    let config: Config = toml::from_str(&content)
        .with_context(|| "配置文件格式错误")?;

    // 提取基础配置
    let unix_hide = config.rule.basic.unix_hide;
    let allowed_exts: HashSet<String> = config.rule.basic.extension
        .into_iter()
        .map(|ext| ext.trim_start_matches('.').to_lowercase())
        .collect();

    // 编译附加规则
    let mut rules = Vec::new();
    for rule in config.rule.attached {
        let compiled = compile_rule(&rule)?;
        rules.push(compiled);
    }

    Ok(AppConfig {
        unix_hide,
        allowed_exts,
        rules,
    })
}

/// 编译单个规则
fn compile_rule(rule: &AttachedRule) -> Result<CompiledRule> {
    let mut name_patterns = Vec::new();
    let mut path_patterns = Vec::new();
    let mut regex_patterns = Vec::new();

    // 解析条件
    match &rule.conditions {
        ConditionUnion::Single { name, path, regex } => {
            if let Some(n) = name {
                name_patterns.extend(extract_patterns(n)?);
            }
            if let Some(p) = path {
                path_patterns.extend(extract_patterns(p)?);
            }
            if let Some(r) = regex {
                regex_patterns.extend(extract_regex_patterns(r)?);
            }
        }
    }

    // 编译所有模式
    let name_matchers = name_patterns
        .iter()
        .map(|p| compile_glob(p))
        .collect::<Result<Vec<_>>>()?;
    
    let path_matchers = path_patterns
        .iter()
        .map(|p| compile_glob(p))
        .collect::<Result<Vec<_>>>()?;
    
    let regex_matchers = regex_patterns
        .iter()
        .map(|p| compile_regex(p))
        .collect::<Result<Vec<_>>>()?;

    Ok(CompiledRule {
        action: rule.action.clone(),
        condition: CompiledCondition {
            name_matchers,
            path_matchers,
            regex_matchers,
        },
    })
}

/// 从 PatternValue 提取字符串模式列表
fn extract_patterns(value: &PatternValue) -> Result<Vec<String>> {
    match value {
        PatternValue::Single(s) => Ok(vec![s.clone()]),
        PatternValue::Array(arr) => Ok(arr.clone()),
        PatternValue::Map(map) => Ok(map.values().cloned().collect()),
    }
}

/// 从 PatternValue 提取正则模式列表
fn extract_regex_patterns(value: &PatternValue) -> Result<Vec<String>> {
    extract_patterns(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = load_config(None).unwrap();
        assert!(config.unix_hide);
        assert!(config.allowed_exts.is_empty());
    }
}
