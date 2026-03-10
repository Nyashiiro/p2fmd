# p2fmd - Project to Flattened Markdown
[![English](https://img.shields.io/badge/Language-EN-blue?style=flat-square)](README-en.md)
[![中文](https://img.shields.io/badge/语言-简中-red?style=flat-square)](README.md)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg?style=flat-square)](LICENSE)


将项目代码文件合并为单个 Markdown 文档，便于分析和文档生成。

## 主要功能

- 将项目中的文本文件合并为一个 Markdown 文件
- 支持通过配置规则包含或排除文件（支持通配符、正则表达式）
- 可检测重复内容并去重
- 支持并行扫描，提高处理速度
- 提供命令行选项，灵活指定目录和输出位置

## 安装

```bash
# 克隆仓库
git clone https://github.com/yourusername/p2fmd.git
cd p2fmd

# 构建发布版本
cargo build --release

# 安装到系统
cargo install --path .
```

## 使用方法

```bash
# 基本用法（当前目录）
p2fmd

# 指定执行目录
p2fmd --execpath /path/to/project

# 指定输出目录
p2fmd --outpath /path/to/output

# 指定配置文件
p2fmd --configpath /path/to/config.toml

# 启用日志记录（崩溃的时候用）
p2fmd --log

# 强制覆盖已有文件
p2fmd --force

# 禁用并行处理（低内存环境）
p2fmd --no-parallel
```

## 配置文件

创建 `p2fmdconfig.toml` 文件来自定义过滤规则：

```toml
[rule.basic]
unix_hide = true
extension = ["rs", "toml", "md"]

[[rule.attached]]
Action = "include"
name = ".gitignore"

[[rule.attached]]
Action = "hide"
name = "*.log"

[[rule.attached]]
Action = "include"
name = "debug.log"
```

详细配置示例请参考 `p2fmdconfig.toml`。

## 项目结构

```
p2fmd/
├── Cargo.toml          # 项目依赖
├── README.md           # 项目说明
├── p2fmdconfig.toml    # 配置示例
└── src/
    ├── main.rs         # 入口模块
    ├── utils.rs        # 工具函数
    ├── config.rs       # 配置解析
    ├── matcher.rs      # 规则匹配
    ├── scanner.rs      # 目录扫描
    └── writer.rs       # 输出生成
```
