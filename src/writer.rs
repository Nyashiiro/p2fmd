//! 输出模块：封装 TeeWriter，负责流式写入文件并同步计算 SHA-256 哈希

use crate::utils::{log_message, sanitize_filename};
use anyhow::Context;
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};

/// 无内存分配 (Zero-Allocation) 的 IO + Hash 包装器
/// 
/// 设计原理：将文件内容流式传输，一边写入磁盘，一边计算哈希，
/// 避免一次性读取大文件导致内存溢出
struct TeeWriter<W: Write> {
    inner: W,
    hasher: Sha256,
}

impl<W: Write> TeeWriter<W> {
    fn new(inner: W) -> Self {
        Self {
            inner,
            hasher: Sha256::new(),
        }
    }

    /// 获取最终的哈希值
    fn finalize(self) -> String {
        format!("{:x}", self.hasher.finalize())
    }
}

impl<W: Write> Write for TeeWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let n = self.inner.write(buf)?;
        // 仅将成功写入的字节送入哈希器
        self.hasher.update(&buf[..n]);
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

/// 生成输出文件并计算内容哈希
pub fn generate_output_and_hash(
    exec_root: &Path,
    matched_files: &[PathBuf],
    out_path: &Path,
    log_enabled: bool,
) -> anyhow::Result<String> {
    // 确保输出目录存在
    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let file = File::create(out_path)
        .with_context(|| format!("无法创建输出文件: {:?}", out_path))?;
    
    let mut writer = TeeWriter::new(BufWriter::new(file));

    // 写入项目标题
    let project_name = exec_root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Project");
    
    writeln!(writer, "# {}\n", project_name)?;

    // 遍历所有匹配的文件
    for (i, path) in matched_files.iter().enumerate() {
        let rel_path = path.strip_prefix(exec_root).unwrap_or(path);
        
        // 写入文件分隔标记
        writeln!(writer, "```{}", rel_path.to_string_lossy())?;

        // 流式读取并写入文件内容
        match File::open(path) {
            Ok(mut file) => {
                // 使用 io::copy 实现零拷贝流式传输
                if let Err(e) = io::copy(&mut file, &mut writer) {
                    let err_msg = format!("[ERROR: 读取文件时出错 {:?}: {}]", path, e);
                    log_message("ERROR", &err_msg);
                    writeln!(writer, "{}", err_msg)?;
                }
            }
            Err(e) => {
                let err_msg = format!("[ERROR: 无法打开文件 {:?}: {}]", path, e);
                log_message("ERROR", &err_msg);
                writeln!(writer, "{}", err_msg)?;
            }
        }

        // 写入文件结束标记
        if i < matched_files.len() - 1 {
            writeln!(writer, "\n```\n---")?;
        } else {
            writeln!(writer, "\n```")?;
        }
    }

    // 如果启用了日志，追加日志内容
    if log_enabled {
        writeln!(writer, "\n\np2fmd-log:")?;
        use crate::utils::LOG_BUFFER;
        let buffer = LOG_BUFFER.lock();
        for log_entry in buffer.iter() {
            writeln!(writer, "{}", log_entry)?;
        }
    }

    // 刷新缓冲区
    writer.flush()?;
    
    Ok(writer.finalize())
}

/// 确定输出文件路径
/// 
/// 基于内容哈希去重，支持强制覆盖
pub fn get_output_path(
    out_dir: &Path,
    exec_root: &Path,
    content_hash: &str,
    force: bool,
) -> anyhow::Result<Option<PathBuf>> {
    // 提前检查输出目录是否可写
    let test_file = out_dir.join(".write_test");
    match fs::write(&test_file, b"test") {
        Ok(_) => {
            let _ = fs::remove_file(&test_file);
        }
        Err(e) => {
            return Err(anyhow::anyhow!(
                "输出目录 {:?} 不可写: {}",
                out_dir, e
            ));
        }
    }

    // 获取项目名
    let raw_name = exec_root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Project");
    
    let folder_name = sanitize_filename(raw_name);
    let base_name = format!("{}_Flattened", folder_name);

    // 检查是否已存在相同内容的文件
    let pattern = format!("{}*.md", base_name);
    let glob_pattern = globset::Glob::new(&pattern)?.compile_matcher();
    
    if let Ok(entries) = fs::read_dir(out_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_file() && glob_pattern.is_match(&path) {
                // 读取现有文件计算哈希（简化处理：比较文件名中的哈希）
                if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                    if filename.contains(&content_hash[..8]) && !force {
                        log_message("INFO", format!(
                            "内容未变化，输出已存在: {:?}",
                            path
                        ));
                        return Ok(None);
                    }
                }
            }
        }
    }

    // 生成不冲突的文件名
    let mut final_name = format!("{}_{}.md", base_name, &content_hash[..8]);
    let mut counter = 2;
    
    while out_dir.join(&final_name).exists() {
        final_name = format!("{}_{}_{}.md", base_name, &content_hash[..8], counter);
        counter += 1;
    }

    Ok(Some(out_dir.join(final_name)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tee_writer() {
        let mut buf = Vec::new();
        {
            let mut writer = TeeWriter::new(&mut buf);
            writer.write_all(b"hello world").unwrap();
            let hash = writer.finalize();
            assert_eq!(hash.len(), 64); // SHA-256 是 64 位十六进制
        }
        assert_eq!(buf, b"hello world");
    }
}
