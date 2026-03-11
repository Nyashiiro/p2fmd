//! 入口模块：参数解析、全局流程控制、信号处理(Ctrl+C)、致命错误捕捉

use anyhow::{Context, Result};
use clap::Parser;
use std::path::{Path, PathBuf};
use std::process;

// 模块声明
mod utils;
mod config;
mod matcher;
mod scanner;
mod writer;

use utils::{Args, GLOBAL_ARGS, LOG_BUFFER, log_message, resolve_path, current_dir, sanitize_filename};
use config::load_config;
use scanner::collect_files;
use writer::{generate_output_and_hash, get_output_path};

/// Project to Flattened Markdown - 高性能项目文件合并工具
#[derive(Parser, Debug, Clone)]
#[command(
    name = "p2fmd",
    version = "1.0.0",
    about = "将项目文件合并为单个 Markdown 文件",
    long_about = "p2fmd (Project to Flattened Markdown) 是一个高性能的命令行工具，\n\
                  用于将项目中的代码文件合并为单个 Markdown 文档，便于 LLM 分析和文档生成。"
)]
struct CliArgs {
    /// 执行目录路径（默认为当前目录）
    #[arg(short = 'e', long = "execpath", value_name = "PATH")]
    execpath: Option<PathBuf>,

    /// 输出目录路径（默认为当前目录）
    #[arg(short = 'o', long = "outpath", value_name = "PATH")]
    outpath: Option<PathBuf>,

    /// 配置文件路径
    #[arg(short = 'c', long = "configpath", value_name = "PATH")]
    configpath: Option<PathBuf>,

    /// 禁用并行处理（用于调试或低内存环境）
    #[arg(long = "no-parallel")]
    no_parallel: bool,

    /// 启用日志记录到输出文件
    #[arg(short = 'l', long = "log")]
    log: bool,

    /// 强制覆盖已有文件
    #[arg(short = 'f', long = "force")]
    force: bool,
}

/// 设置 Ctrl+C 信号处理器
fn setup_ctrlc_handler() {
    ctrlc::set_handler(move || {
        eprintln!("\n[INFO] 接收到中断信号 (Ctrl+C)，正在保存当前日志...");
        
        let buffer = LOG_BUFFER.lock();
        let logs: Vec<String> = buffer.iter().cloned().collect();
        let dump_path = std::env::temp_dir().join("p2fmd_interrupted.log");
        
        match std::fs::write(&dump_path, logs.join("\n")) {
            Ok(_) => {
                eprintln!("[INFO] 退出日志已保存至: {:?}", dump_path);
            }
            Err(e) => {
                eprintln!("[WARN] 无法保存退出日志: {}", e);
            }
        }
        
        process::exit(130);
    }).expect("设置 Ctrl-C 处理器失败");
}

/// 处理致命错误
fn handle_fatal_error(err: anyhow::Error) {
    eprintln!("[FATAL] 程序异常终止: {}", err);
    
    if let Some(args) = GLOBAL_ARGS.get() {
        if args.log {
            // 确定项目名
            let project_name = args.execpath
                .as_ref()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .map(|s| s.to_string())
                .or_else(|| {
                    let cwd = std::env::current_dir().ok()?;
                    cwd.file_name()
                        .and_then(|n| n.to_str())
                        .map(|s| s.to_string())
                })
                .unwrap_or_else(|| "Fatal".to_string());
            
            let sanitized = sanitize_filename(&project_name);
            
            // 确定输出目录
            let out_dir = args.outpath
                .as_ref()
                .filter(|p| p.is_dir())
                .cloned()
                .unwrap_or_else(std::env::temp_dir);
            
            // 生成不冲突的错误日志文件名
            let mut counter = 0;
            let dump_path = loop {
                let name = if counter == 0 {
                    format!("{}_Flattened_ERROR.log", sanitized)
                } else {
                    format!("{}_Flattened_ERROR_{}.log", sanitized, counter)
                };
                let path = out_dir.join(&name);
                if !path.exists() {
                    break path;
                }
                counter += 1;
            };
            
            // 构造日志内容
            let buffer = LOG_BUFFER.lock();
            let logs: Vec<String> = buffer.iter().cloned().collect();
            let content = format!(
                "FATAL ERROR: {}\n\nLOGS:\n{}",
                err,
                logs.join("\n")
            );
            
            // 尝试写入文件
            if std::fs::write(&dump_path, content).is_ok() {
                eprintln!("[INFO] 崩溃日志已保存至: {:?}", dump_path);
            }
        }
    }
}

/// 定位配置文件
fn locate_config(
    config_param: Option<&Path>,
    exec_root: &Path,
    cwd: &Path,
) -> Result<Option<PathBuf>> {
    // 默认配置文件名
    const DEFAULT_CONFIG: &str = "p2fmdconfig.toml";

    // 如果未指定配置路径，在 exec_root 下查找默认配置
    if config_param.is_none() {
        let default_path = exec_root.join(DEFAULT_CONFIG);
        if default_path.is_file() {
            return Ok(Some(default_path));
        }
        return Ok(None);
    }

    // 解析用户指定的路径
    let abs_path = resolve_path(config_param.unwrap(), cwd);

    if abs_path.is_dir() {
        // 如果是目录，查找目录下的默认配置
        let config_in_dir = abs_path.join(DEFAULT_CONFIG);
        if config_in_dir.is_file() {
            Ok(Some(config_in_dir))
        } else {
            log_message("WARN", format!(
                "指定目录 {:?} 下未找到配置文件，使用默认配置",
                abs_path
            ));
            Ok(None)
        }
    } else if abs_path.is_file() {
        // 如果是文件，直接使用
        Ok(Some(abs_path))
    } else {
        // 路径不存在
        Err(anyhow::anyhow!(
            "指定的配置文件路径不存在: {:?}",
            abs_path
        ))
    }
}

/// 主运行逻辑
fn run() -> Result<()> {
    // 1. 解析命令行参数
    let cli_args = CliArgs::parse();
    
    // 转换为内部 Args 结构并存储到全局
    let args = Args {
        execpath: cli_args.execpath.clone(),
        outpath: cli_args.outpath.clone(),
        configpath: cli_args.configpath.clone(),
        no_parallel: cli_args.no_parallel,
        log: cli_args.log,
        force: cli_args.force,
    };
    
    GLOBAL_ARGS.set(args.clone())
        .map_err(|_| anyhow::anyhow!("无法初始化全局参数"))?;

    // 2. 获取当前工作目录
    let cwd = current_dir()?;

    // 3. 解析执行目录
    let exec_root = if let Some(ref exec_path) = args.execpath {
        let resolved = resolve_path(exec_path, &cwd);
        if !resolved.exists() {
            return Err(anyhow::anyhow!(
                "执行目录不存在: {:?}",
                exec_path
            ));
        }
        if !resolved.is_dir() {
            return Err(anyhow::anyhow!(
                "执行路径必须是目录: {:?}",
                exec_path
            ));
        }
        resolved
    } else {
        cwd.clone()
    };

    log_message("INFO", format!("执行目录: {:?}", exec_root));

    // 4. 解析输出目录
    let out_dir = if let Some(ref out_path) = args.outpath {
        let resolved = resolve_path(out_path, &cwd);
        if !resolved.exists() {
            std::fs::create_dir_all(&resolved)
                .with_context(|| format!("无法创建输出目录: {:?}", resolved))?;
        }
        if !resolved.is_dir() {
            return Err(anyhow::anyhow!(
                "输出路径必须是目录: {:?}",
                out_path
            ));
        }
        resolved
    } else {
        cwd.clone()
    };

    log_message("INFO", format!("输出目录: {:?}", out_dir));

    // 5. 定位配置文件
    let config_path = locate_config(
        args.configpath.as_deref(),
        &exec_root,
        &cwd
    )?;

    if let Some(ref path) = config_path {
        log_message("INFO", format!("使用配置文件: {:?}", path));
    }

    // 6. 加载配置
    let config = load_config(config_path.as_deref())?;

    if config.allowed_exts.is_empty() {
        log_message("WARN", "未配置任何文件扩展名限制，将包含所有文件");
    } else {
        log_message("INFO", format!(
            "允许的文件扩展名: {:?}",
            config.allowed_exts
        ));
    }

    // 7. 收集文件
    log_message("INFO", "开始扫描文件树...");
    
    let matched_files = collect_files(&exec_root, &config, args.no_parallel)?;

    if matched_files.is_empty() {
        log_message("INFO", "没有找到匹配的文件。");
        return Ok(());
    }

    log_message("INFO", format!(
        "共找到 {} 个需处理的文件",
        matched_files.len()
    ));

    // 8. 创建临时输出文件
    let temp_path = out_dir.join(format!(".tmp_{}.md", process::id()));
    
    log_message("INFO", "正在生成输出文件...");

    // 9. 生成输出并计算哈希
    let content_hash = generate_output_and_hash(
        &exec_root,
        &matched_files,
        &temp_path,
        args.log,
    )?;

    log_message("INFO", format!("内容哈希: {}", &content_hash[..16]));

    // 10. 确定最终输出路径
    let final_path_opt = get_output_path(
        &out_dir,
        &exec_root,
        &content_hash,
        args.force,
    )?;

    // 11. 处理输出文件
    match final_path_opt {
        Some(final_path) => {
            // 重命名临时文件为最终文件名
            std::fs::rename(&temp_path, &final_path)
                .with_context(|| format!(
                    "无法重命名文件: {:?} -> {:?}",
                    temp_path, final_path
                ))?;
            
            log_message("INFO", format!(
                "成功生成合并文件: {:?}",
                final_path
            ));
        }
        None => {
            // 内容重复，删除临时文件
            let _ = std::fs::remove_file(&temp_path);
            log_message("INFO", "内容未变化，跳过输出");
        }
    }

    Ok(())
}

fn main() {
    // 设置信号处理器
    setup_ctrlc_handler();

    // 运行主逻辑
    match run() {
        Ok(()) => {
            process::exit(0);
        }
        Err(e) => {
            handle_fatal_error(e);
            process::exit(1);
        }
    }
}
