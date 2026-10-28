//!
//! # 性能冒烟（plan Task 8 Step 4）
//!
//! 合成语料（约 500KB、约 2000 节点、50 表格、100 链接）过完整
//! [`render_pipeline`]，debug 构建断言 1s 预算（防解析/树构建意外劣化）。
//! release 100ms 预算由 Task 14 在 CI/release 模式验证——本测试不设 release
//! 断言（brief 形态：单条 1s 断言），避免 CI 机器噪声造成假失败。

use std::time::{Duration, Instant};

use postium_html::render_pipeline;
use postium_html::types::{Block, InlineRun, SafeUrl};

/// 合成语料：50 张两列表格（每表 2 个 http 链接 → 100 链接），单元格内长段落
/// 与表后填充段落补足字节预算；节点规模约 2000（元素 + 文本，构造估算），远低于
/// `max_nodes`（20_000），字节量低于 `max_bytes`（1MB），两道限制均不触发。
fn synthetic_html() -> String {
    const TABLES: usize = 50;
    const TARGET_BYTES: usize = 500_000;
    /// 单句约 117 字节（UTF-8），按倍数拼出填充段落。
    const SENTENCE: &str =
        "邮件正文合成语料：块级布局与表格链接混合排布，覆盖段落、行内样式与宽度背景属性。";
    let unit_filler = SENTENCE.repeat(12);
    let pad_filler = SENTENCE.repeat(6);

    let mut html = String::with_capacity(TARGET_BYTES + 4096);
    html.push_str(r#"<div style="max-width:600px;margin:0 auto;font-size:14px">"#);
    for i in 0..TABLES {
        html.push_str(&format!(
            r##"<table width="600" bgcolor="#ffffff"><tr><td width="50%" bgcolor="#eeeeee" style="padding:8px"><p style="margin:0 0 8px 0">{unit_filler}</p><p>第{i}节 <a href="https://example.com/item/{i}">条目{i}</a></p></td><td width="50%" style="padding:8px"><p style="margin:0 0 8px 0;color:#333333">{unit_filler}</p><p><a href="https://example.com/more/{i}">延伸阅读{i}</a></p></td></tr></table>"##,
        ));
    }
    // 字节预算补齐：小段落追加至 ≥ 500KB（节点数同步增长，构造估算共约 2000）
    while html.len() < TARGET_BYTES {
        html.push_str(&format!(r#"<p style="margin:0">{pad_filler}</p>"#));
    }
    html.push_str("</div>");
    html
}

/// 递归统计块级树中的表格数与 http 链接数（链接内无块级内容，不递归 runs）。
fn count_tables_links(blocks: &[Block], tables: &mut usize, links: &mut usize) {
    for block in blocks {
        match block {
            Block::Table { rows, .. } => {
                *tables += 1;
                for row in rows {
                    for cell in &row.cells {
                        count_tables_links(&cell.blocks, tables, links);
                    }
                }
            }
            Block::Container { children, .. } | Block::Quote { children } => {
                count_tables_links(children, tables, links)
            }
            Block::List { items, .. } => {
                for item in items {
                    count_tables_links(item, tables, links);
                }
            }
            Block::Paragraph { runs, .. } | Block::Heading { runs, .. } => {
                for run in runs {
                    if let InlineRun::Link {
                        href: SafeUrl::Http(_),
                        ..
                    } = run
                    {
                        *links += 1;
                    }
                }
            }
            Block::Image { .. } | Block::Hr => {}
        }
    }
}

#[test]
fn pipeline_500kb_under_budget() {
    let html = synthetic_html();
    // 语料自检：≥ 500KB（brief 合成语料规格的上界防手滑）
    assert!(
        (490_000..510_000).contains(&html.len()),
        "合成语料应约 500KB，实际 {} 字节",
        html.len()
    );

    let start = Instant::now();
    let doc = render_pipeline(&html).unwrap();
    let el = start.elapsed();
    assert!(
        el < Duration::from_millis(1000),
        "debug 预算 1s，实际 {el:?}"
    );
    // release 100ms 预算由 Task 14 在 CI/release 模式验证

    // 结构自检：50 表格、100 链接确实进入 IR（生成器拼写错误的保险）
    let (mut tables, mut links) = (0, 0);
    count_tables_links(&doc.blocks, &mut tables, &mut links);
    assert_eq!((tables, links), (50, 100), "合成语料应含 50 表格、100 链接");
}
