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
    assert!(paths.len() >= 3, "fixture 语料应至少 3 封");

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
