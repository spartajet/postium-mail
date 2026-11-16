//! 标记序列化快照：全部 fixture 经完整管线 → IR → serialize_marker_html，
//! 输出字节级锁定（MailView 路线的回归护栏——顺序、数量、无内容泄漏）。

use std::fs;
use std::path::{Path, PathBuf};

use postium_html::{render_pipeline, serialize_marker_html};

const FIXTURES_DIR: &str = "tests/fixtures";
const SNAPSHOTS_DIR: &str = "tests/fixtures/markers";

fn fixture_paths() -> Vec<PathBuf> {
    let mut paths: Vec<_> = fs::read_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURES_DIR))
        .expect("tests/fixtures 目录应存在")
        .map(|entry| entry.expect("目录项应可读").path())
        .filter(|path| path.is_file() && path.extension().is_some_and(|ext| ext == "html"))
        .collect();
    paths.sort();
    paths
}

#[test]
fn fixtures_match_marker_snapshots() {
    let update = std::env::var("UPDATE_SNAPSHOTS").as_deref() == Ok("1");
    let snapshots_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(SNAPSHOTS_DIR);
    let paths = fixture_paths();
    assert!(paths.len() >= 10, "fixture 语料应至少 10 封");

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
        let actual = serialize_marker_html(&doc.blocks);

        let snapshot = snapshots_dir.join(format!("{name}.txt"));
        if update {
            fs::create_dir_all(&snapshots_dir).expect("快照目录应可创建");
            fs::write(&snapshot, &actual).unwrap_or_else(|err| panic!("{name}: 快照应可写：{err}"));
            println!("{name}: 标记快照已更新（{} 字节）", actual.len());
            continue;
        }

        let expected = fs::read_to_string(&snapshot).unwrap_or_else(|err| {
            panic!(
                "{name}: 标记快照缺失（{err}）——先运行 \
                 `UPDATE_SNAPSHOTS=1 cargo test -p postium-html --test markers` 生成并人工审阅"
            )
        });
        assert_eq!(
            actual, expected,
            "{name}: 标记快照不一致——序列化顺序/形态变更需人工审阅"
        );

        // 不变式：每个 fixture 至少一个标记；输出不含任何正文内容（内容只走 IR 通道）。
        let marker_count = actual.lines().count();
        assert!(marker_count >= 1, "{name}: 标记数应 ≥1");
        for line in actual.lines() {
            assert!(
                line.starts_with("<div data-mb=\"") && line.ends_with("\"></div>"),
                "{name}: 标记形态异常：{line}"
            );
        }
    }
}
