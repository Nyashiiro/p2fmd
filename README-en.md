# p2fmd - Project to Flattened Markdown
[![English](https://img.shields.io/badge/Language-EN-blue?style=flat-square)](README-en.md)
[![中文](https://img.shields.io/badge/语言-简中-red?style=flat-square)](README.md)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg?style=flat-square)](LICENSE)

Merge project code files into a single Markdown document for easy analysis and documentation generation.

## Features

- Merge text files in a project into one Markdown file
- Support including/excluding files through configuration rules (glob patterns, regular expressions)
- Detect duplicate content and deduplicate
- Support parallel scanning for improved processing speed
- Provide command-line options to flexibly specify directories and output location

## Installation

```bash
# Clone the repository
git clone https://github.com/yourusername/p2fmd.git
cd p2fmd

# Build the release version
cargo build --release

# Install to system
cargo install --path .
```

## Usage

```bash
# Basic usage (current directory)
p2fmd

# Specify execution directory
p2fmd --execpath /path/to/project

# Specify output directory
p2fmd --outpath /path/to/output

# Specify configuration file
p2fmd --configpath /path/to/config.toml

# Enable logging (useful for debugging crashes)
p2fmd --log

# Force overwrite existing file
p2fmd --force

# Disable parallel processing (for low-memory environments)
p2fmd --no-parallel
```

## Configuration

Create a `p2fmdconfig.toml` file to customize filtering rules:

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

See `p2fmdconfig.toml` for a detailed configuration example.

## Project Structure

```
p2fmd/
├── Cargo.toml          # Project dependencies
├── README.md           # Project documentation
├── p2fmdconfig.toml    # Example configuration
└── src/
    ├── main.rs         # Entry module
    ├── utils.rs        # Utility functions
    ├── config.rs       # Configuration parsing
    ├── matcher.rs      # Rule matching
    ├── scanner.rs      # Directory scanning
    └── writer.rs       # Output generation
```
