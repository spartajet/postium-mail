//! 构建脚本：从工作区根目录的 `.env` 读取 OAuth2 等应用凭据，
//! 通过 `cargo:rustc-env` 注入编译期环境变量，供源码中的 `option_env!`
//! 读取（providers 的 MICROSOFT_CLIENT_ID / GOOGLE_CLIENT_ID 等）。
//!
//! `.env` 不进版本库（见 .gitignore）；模板见 `.env.example`。

use std::path::PathBuf;
use std::process::exit;

fn main() {
    // 从本 crate 目录向上逐级查找 .env（约定放工作区根）
    let manifest_dir = PathBuf::from(
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR 未设置"),
    );
    let mut env_file = None;
    let mut dir = Some(manifest_dir.as_path());
    while let Some(d) = dir {
        let candidate = d.join(".env");
        if candidate.is_file() {
            env_file = Some(candidate);
            break;
        }
        dir = d.parent();
    }
    let env_file = env_file.unwrap_or_else(|| {
        // 找不到时以工作区根为监视目标（创建 .env 后能触发重编）
        manifest_dir
            .parent()
            .map(|p| p.join(".env"))
            .unwrap_or_else(|| manifest_dir.join(".env"))
    });

    // .env 变化时重新编译（不存在时也监听，创建后能触发构建）
    println!("cargo:rerun-if-changed={}", env_file.display());
    println!("cargo:rerun-if-changed=build.rs");

    let Ok(content) = std::fs::read_to_string(&env_file) else {
        // 没有 .env 不视为错误：编译可继续，OAuth2 启动时由
        // ensure_credentials 给出带指引的明确错误
        eprintln!("build.rs: 未找到 .env（从 {} 向上查找）", manifest_dir.display());
        return;
    };

    for (line_no, raw) in content.lines().enumerate() {
        let line = raw.trim();
        // 跳过空行与注释
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            eprintln!("build.rs: .env 第 {} 行缺少 '='，已忽略：{raw}", line_no + 1);
            continue;
        };
        let key = key.trim();
        let mut value = value.trim();
        // 支持内联注释（无引号时）与成对引号
        if let Some(hash) = value.find(" #") {
            value = value[..hash].trim();
        }
        if (value.starts_with('"') && value.ends_with('"') && value.len() >= 2)
            || (value.starts_with('\'') && value.ends_with('\'') && value.len() >= 2)
        {
            value = &value[1..value.len() - 1];
        }
        if key.is_empty() || value.is_empty() {
            continue;
        }
        // 仅注入凭据类键，避免 .env 里的无关变量泄漏进构建环境
        if key.ends_with("_CLIENT_ID") || key.ends_with("_CLIENT_SECRET") {
            println!("cargo:rustc-env={key}={value}");
        }
    }

    // 编译期静默凭据是易错点：没有注入任何 *_CLIENT_ID 时给出醒目提示
    let injected_any = [
        "MICROSOFT_CLIENT_ID",
        "GOOGLE_CLIENT_ID",
    ]
    .iter()
    .any(|k| std::env::var(k).map(|v| !v.is_empty()).unwrap_or(false));
    if !injected_any {
        eprintln!(
            "build.rs: 工作区根 .env 中未找到有效的 OAuth2 客户端 ID\n\
             （需要 MICROSOFT_CLIENT_ID / GOOGLE_CLIENT_ID）。\n\
             OAuth2 登录将在启动授权时返回明确错误；密码型登录不受影响。"
        );
        // 不 exit(1)：保持其他功能可构建/可测试
        let _ = exit;
    }
}
