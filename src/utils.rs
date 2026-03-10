//! 工具模块：全局日志缓冲区、路径规范化、文件名净化等纯函数

use parking_lot::Mutex;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// 全局命令行参数存储（使用 OnceLock 实现线程安全的懒加载）
pub static GLOBAL_ARGS: OnceLock<Args> = OnceLock::new();

/// 循环日志缓冲区（高性能锁保护）
pub static LOG_BUFFER: Mutex<VecDeque<String>> = Mutex::new(VecDeque::new());

/// 命令行参数结构体
#[derive(Debug, Clone)]
pub struct Args {
    /// 执行目录路径
    pub execpath: Option<PathBuf>,
    /// 输出目录路径
    pub outpath: Option<PathBuf>,
    /// 配置文件路径
    pub configpath: Option<PathBuf>,
    /// 禁用并行处理
    pub no_parallel: bool,
    /// 启用日志记录
    pub log: bool,
    /// 强制覆盖已有文件
    pub force: bool,
}

/// 记录日志消息到控制台和循环缓冲区
pub fn log_message(level: &str, message: impl AsRef<str>) {
    let msg = format!("[{}] {}", level, message.as_ref());
    eprintln!("{}", msg);
    
    let mut buffer = LOG_BUFFER.lock();
    if buffer.len() >= 1000 {
        buffer.pop_front();
    }
    buffer.push_back(msg);
}

/// 净化字符串，使其适合作为文件名
/// 只保留字母、数字、下划线、连字符，截断至 64 字符
pub fn sanitize_filename(s: &str) -> String {
    let mut clean: String = s
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    clean.truncate(64);
    clean
}

/// 将用户输入的路径（相对或绝对）转换为规范化的绝对路径
pub fn resolve_path(input: &Path, base: &Path) -> PathBuf {
    if input.is_absolute() {
        input.to_path_buf()
    } else {
        let joined = base.join(input);
        // 尝试规范化路径，失败则返回原始拼接路径
        joined.canonicalize().unwrap_or(joined)
    }
}

/// 判断路径中是否有任何组件以点开头（Unix 隐藏文件/目录）
pub fn has_hidden_component(path: &Path) -> bool {
    path.components().any(|comp| {
        comp.as_os_str()
            .to_str()
            .map(|s| s.starts_with('.'))
            .unwrap_or(false)
    })
}

/// 获取当前工作目录
pub fn current_dir() -> anyhow::Result<PathBuf> {
    std::env::current_dir().map_err(|e| anyhow::anyhow!("无法获取当前工作目录: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("hello world"), "hello_world");
        assert_eq!(sanitize_filename("test/file:name"), "test_file_name");
        assert_eq!(sanitize_filename(&"a".repeat(100)), "a".repeat(64));
    }

    #[test]
    fn test_has_hidden_component() {
        assert!(has_hidden_component(Path::new(".git/config")));
        assert!(has_hidden_component(Path::new("src/.hidden")));
        assert!(!has_hidden_component(Path::new("src/main.rs")));
    }
}
