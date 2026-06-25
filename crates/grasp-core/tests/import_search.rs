//! End-to-end: a tiny synthetic transcript → import → search.

use std::io::Write;

use grasp_core::config::Config;
use grasp_core::store::db;
use grasp_core::{import, store};

fn write_transcript(dir: &std::path::Path) {
    std::fs::create_dir_all(dir).unwrap();
    let mut f = std::fs::File::create(dir.join("session.jsonl")).unwrap();
    // A messy mix: unknown types, a decision, a file write, an error + fix.
    let lines = [
        r#"{"type":"queue-operation","operation":"enqueue"}"#,
        r#"{"type":"user","timestamp":"2026-01-15T10:00:00Z","sessionId":"abc","message":{"role":"user","content":"why is the auth token expiring early?"}}"#,
        r#"{"type":"assistant","timestamp":"2026-01-15T10:01:00Z","sessionId":"abc","message":{"role":"assistant","content":[{"type":"text","text":"We decided to use a 30s timeout because the REST client had none."},{"type":"tool_use","name":"Edit","input":{"file_path":"src/auth.rs"}}]}}"#,
        r#"{"type":"file-history-snapshot"}"#,
        r#"{"type":"user","timestamp":"2026-01-15T10:02:00Z","sessionId":"abc","message":{"role":"user","content":[{"type":"tool_result","is_error":true,"content":"context deadline exceeded"}]}}"#,
        r#"{"type":"assistant","timestamp":"2026-01-15T10:03:00Z","sessionId":"abc","message":{"role":"assistant","content":[{"type":"text","text":"Added the missing timeout to the kubeconfig loader."}]}}"#,
    ];
    for l in lines {
        writeln!(f, "{l}").unwrap();
    }
}

#[test]
fn import_then_search_roundtrip() {
    let tmp = std::env::temp_dir().join(format!("grasp-it-{}", std::process::id()));
    let projects = tmp.join("projects").join("c--projects-demo");
    write_transcript(&projects);

    let config = Config {
        claude_projects_dir: tmp.join("projects"),
        data_dir: tmp.join("data"),
    };
    let conn = db::open(&config).unwrap();

    let report = import::import_all(&conn, &config, None).unwrap();
    assert_eq!(report.files_processed, 1);
    assert!(report.chunks_added >= 3, "expected several chunks");

    // BM25 search finds the decision.
    let hits = store::index::search(&conn, "timeout REST client", None, 5).unwrap();
    assert!(hits.iter().any(|h| h.text.contains("timeout")));

    // Re-import is a near no-op (file unchanged → skipped, no new chunks).
    let report2 = import::import_all(&conn, &config, None).unwrap();
    assert_eq!(report2.files_skipped, 1);
    assert_eq!(report2.chunks_added, 0);

    // Markdown source-of-truth file exists.
    let md_dir = config.memory_project_dir("c--projects-demo");
    let any_md = std::fs::read_dir(&md_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .any(|e| e.path().extension().and_then(|x| x.to_str()) == Some("md"));
    assert!(any_md, "expected a markdown file in {}", md_dir.display());

    std::fs::remove_dir_all(&tmp).ok();
}
