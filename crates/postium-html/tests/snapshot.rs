//!
//! # JSON 快照测试（plan Task 8）：全管线串联验证
//!
//! 真实结构的手工 fixture（`tests/fixtures/*.html`）→ [`render_pipeline`] 完整
//! 管线（MSO 剥离 → 样式内联 → 净化 → 树构建）→ [`HtmlDocument`] 中间表示 →
//! serde_json pretty 序列化，与 `tests/fixtures/snapshots/<name>.json` 逐字节比对。
//!
//! - 序列化形态固定为 **pretty**（供人工审阅 IR 结构合理性，Task 8 Step 3）；
//! - 快照缺失/不一致 → 测试失败，失败信息附生成命令；
//! - 重新生成：`UPDATE_SNAPSHOTS=1 cargo test -p postium-html --test snapshot`
//!   （`std::env::var` 读取环境开关，无额外依赖）；生成后须人工审阅差异。

use std::fs;
use std::path::{Path, PathBuf};

use postium_html::types::{Block, ImageRef, InlineRun, SafeUrl};
use postium_html::render_pipeline;

/// fixture 目录（.html 语料）。
const FIXTURES_DIR: &str = "tests/fixtures";
/// 快照目录（`<name>.json` 与 fixture 同名）。
const SNAPSHOTS_DIR: &str = "tests/fixtures/snapshots";

/// 收集 fixtures 目录下的 .html 语料（文件名排序，断言顺序稳定）。
fn fixture_paths() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURES_DIR);
    let mut paths: Vec<PathBuf> = fs::read_dir(&dir)
        .expect("tests/fixtures 目录应存在")
        .map(|entry| entry.expect("目录项应可读").path())
        .filter(|path| path.is_file() && path.extension().is_some_and(|ext| ext == "html"))
        .collect();
    paths.sort();
    paths
}

#[test]
fn fixtures_match_json_snapshots() {
    let update = std::env::var("UPDATE_SNAPSHOTS").as_deref() == Ok("1");
    let snapshots_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(SNAPSHOTS_DIR);
    let paths = fixture_paths();
    assert!(paths.len() >= 10, "fixture 语料应至少 10 封（Task 14 扩充）");

    for path in &paths {
        let name = path
            .file_stem()
            .expect("fixture 应有文件名")
            .to_string_lossy()
            .into_owned();
        let html =
            fs::read_to_string(path).unwrap_or_else(|err| panic!("{name}: fixture 应可读：{err}"));
        let doc = render_pipeline(&html)
            .unwrap_or_else(|err| panic!("{name}: 完整管线应成功，实际 {err:?}"));
        let actual = serde_json::to_string_pretty(&doc)
            .unwrap_or_else(|err| panic!("{name}: IR 应可序列化：{err}"));

        let snapshot = snapshots_dir.join(format!("{name}.json"));
        if update {
            fs::create_dir_all(&snapshots_dir).expect("快照目录应可创建");
            fs::write(&snapshot, &actual).unwrap_or_else(|err| panic!("{name}: 快照应可写：{err}"));
            println!("{name}: 快照已更新（{} 字节）", actual.len());
            continue;
        }

        let expected = fs::read_to_string(&snapshot).unwrap_or_else(|err| {
            panic!(
                "{name}: 快照缺失（{err}）——先人工审阅 IR 序列化输出，确认结构合理后运行 \
                 `UPDATE_SNAPSHOTS=1 cargo test -p postium-html --test snapshot` 生成"
            )
        });
        assert_eq!(
            actual, expected,
            "{name}: IR 快照不一致——若为预期中的行为变更，请更新快照并人工审阅差异"
        );
    }
}

// ---------------------------------------------------------------------------
// 语料级断言（Task 14 扩充）：快照逐字节比对之外，对新语料的安全/深色语义
// 加显式断言——快照审阅是人工环节，这两类红线需要机器可复核的守卫。
// ---------------------------------------------------------------------------

/// 读语料并过完整管线（`tests/fixtures/<name>.html`）。
fn pipeline_fixture(name: &str) -> postium_html::HtmlDocument {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(FIXTURES_DIR)
        .join(format!("{name}.html"));
    let html = fs::read_to_string(&path).unwrap_or_else(|err| panic!("{name}: 语料应可读：{err}"));
    render_pipeline(&html).unwrap_or_else(|err| panic!("{name}: 管线应成功：{err:?}"))
}

/// 深色底原生邮件（dark-native）：`needs_dark_adapt == false`——深色模式下
/// 原样 clone（spec §6 触发判定），不做亮度反转。
#[test]
fn fixture_dark_native_needs_no_dark_adapt() {
    let doc = pipeline_fixture("dark-native");
    assert!(
        !postium_html::needs_dark_adapt(&doc),
        "深底原生邮件不应触发深色适配"
    );
    assert_eq!(
        postium_html::adapt_document(&doc, true),
        doc,
        "深底文档 adapt(dark=true) 应原样 clone"
    );
}

/// 浅底营销语料（marketing-nested-table）：反向对照——需要深色适配（首显式
/// 背景 #eef1f5 亮度 > 0.6）。
#[test]
fn fixture_marketing_nested_table_needs_dark_adapt() {
    let doc = pipeline_fixture("marketing-nested-table");
    assert!(postium_html::needs_dark_adapt(&doc));
}

/// 递归收集链接 href 与图片来源分类标记（嵌套容器/引用块/表格单元格/列表项
/// 全覆盖；行内 run 与链接内层递归）。
fn collect_urls(blocks: &[Block], links: &mut Vec<String>, images: &mut Vec<String>) {
    for block in blocks {
        match block {
            Block::Container { children, .. } | Block::Quote { children } => {
                collect_urls(children, links, images)
            }
            Block::Paragraph { runs, .. } | Block::Heading { runs, .. } => {
                collect_run_urls(runs, links, images)
            }
            Block::List { items, .. } => {
                for item in items {
                    collect_urls(item, links, images);
                }
            }
            Block::Table { rows, .. } => {
                for row in rows {
                    for cell in &row.cells {
                        collect_urls(&cell.blocks, links, images);
                    }
                }
            }
            Block::Image { src, .. } => images.push(image_tag(src)),
            Block::Hr => {}
        }
    }
}

fn collect_run_urls(runs: &[InlineRun], links: &mut Vec<String>, images: &mut Vec<String>) {
    for run in runs {
        match run {
            InlineRun::Link { href, runs } => {
                links.push(match href {
                    SafeUrl::Http(u) | SafeUrl::Mailto(u) => u.clone(),
                });
                collect_run_urls(runs, links, images);
            }
            InlineRun::Image { src, .. } => images.push(image_tag(src)),
            InlineRun::Text { .. } | InlineRun::Br => {}
        }
    }
}

/// 图片来源 → 可断言的分类标记（Remote 带 URL 前缀，其余按变体）。
fn image_tag(src: &ImageRef) -> String {
    match src {
        ImageRef::Remote { url } => format!("remote:{url}"),
        ImageRef::Cid { .. } => "cid".into(),
        ImageRef::DataUri { .. } => "data".into(),
        ImageRef::Invalid => "invalid".into(),
    }
}

/// 攻击语料（attack-non-whitelist）：白名单外资源终审分类——
/// - 链接：javascript:/vbscript:/data:/相对路径 href 一律降级纯文本
///   （实体变体经 html5ever 解码后同样拒绝）；合法 https 链接保留；
/// - 图片：data:text 与 file:// 归 [`ImageRef::Invalid`]；未随附 cid 归
///   [`ImageRef::Cid`]（渲染期按附件匹配，未命中走占位）；
/// - 文本：被降级链接的可见文字保留（内容不丢）。
#[test]
fn fixture_attack_resources_classified_unsafe() {
    let doc = pipeline_fixture("attack-non-whitelist");
    let (mut links, mut images) = (Vec::new(), Vec::new());
    collect_urls(&doc.blocks, &mut links, &mut images);

    for href in &links {
        assert!(
            href.starts_with("https://"),
            "链接 href 应全为 https 白名单，实际 {href}"
        );
    }
    assert!(
        links.contains(&"https://ok.postium.example/safe".to_string()),
        "本体合法的链接应保留（onclick 属性由净化剥离）"
    );

    let invalid = images.iter().filter(|t| t.as_str() == "invalid").count();
    let cids = images.iter().filter(|t| t.as_str() == "cid").count();
    assert_eq!(
        (invalid, cids),
        (2, 1),
        "data:text 与 file:// 应归 Invalid、未随附 cid 归 Cid，实际 {images:?}"
    );
    assert!(
        !images.iter().any(|t| t.starts_with("remote:file")),
        "file 协议不得以 Remote 形态存活"
    );

    // 降级链接的可见文字保留在 IR 文本 run 中（结构不丢内容）
    let text = doc_text(&doc.blocks);
    for visible in ["点我领奖", "实体编码", "VB 脚本", "data 链接", "站内相对路径"] {
        assert!(text.contains(visible), "降级链接文字应保留：{visible}");
    }
}

/// 递归收集全部文本 run 内容（供降级文字保留断言）。
fn doc_text(blocks: &[Block]) -> String {
    fn walk(blocks: &[Block], out: &mut String) {
        for block in blocks {
            match block {
                Block::Paragraph { runs, .. } | Block::Heading { runs, .. } => walk_runs(runs, out),
                Block::Container { children, .. } | Block::Quote { children } => {
                    walk(children, out)
                }
                Block::List { items, .. } => {
                    for item in items {
                        walk(item, out);
                    }
                }
                Block::Table { rows, .. } => {
                    for row in rows {
                        for cell in &row.cells {
                            walk(&cell.blocks, out);
                        }
                    }
                }
                Block::Image { alt, .. } => out.push_str(alt),
                Block::Hr => {}
            }
        }
    }
    fn walk_runs(runs: &[InlineRun], out: &mut String) {
        for run in runs {
            match run {
                InlineRun::Text { text, .. } => out.push_str(text),
                InlineRun::Link { runs, .. } => walk_runs(runs, out),
                InlineRun::Image { alt, .. } => out.push_str(alt),
                InlineRun::Br => {}
            }
        }
    }
    let mut out = String::new();
    walk(blocks, &mut out);
    out
}
