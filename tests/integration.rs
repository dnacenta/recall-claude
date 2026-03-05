use std::fs;
use std::io::Write;
use std::path::Path;

fn write_synthetic_jsonl(dir: &Path, session_id: &str, user_msg: &str) -> String {
    let path = dir.join(format!("{session_id}.jsonl"));
    let mut f = fs::File::create(&path).unwrap();
    let lines = [
        format!(
            r#"{{"type":"queue-operation","operation":"enqueue","timestamp":"2026-03-05T14:30:00.000Z","sessionId":"{session_id}"}}"#,
        ),
        format!(
            r#"{{"type":"queue-operation","operation":"dequeue","timestamp":"2026-03-05T14:30:00.001Z","sessionId":"{session_id}"}}"#,
        ),
        format!(
            r#"{{"parentUuid":null,"type":"user","sessionId":"{session_id}","timestamp":"2026-03-05T14:30:00.100Z","message":{{"role":"user","content":"{user_msg}"}}}}"#,
        ),
        format!(
            r#"{{"parentUuid":"aaa","type":"assistant","sessionId":"{session_id}","timestamp":"2026-03-05T14:30:05.000Z","message":{{"role":"assistant","content":[{{"type":"thinking","thinking":"Let me think about this.","signature":"sig123"}}]}}}}"#,
        ),
        format!(
            r#"{{"parentUuid":"bbb","type":"assistant","sessionId":"{session_id}","timestamp":"2026-03-05T14:31:00.000Z","message":{{"role":"assistant","content":[{{"type":"text","text":"Here is my response to your question."}}]}}}}"#,
        ),
    ];
    for line in &lines {
        writeln!(f, "{}", line).unwrap();
    }
    path.to_string_lossy().to_string()
}

#[test]
fn full_lifecycle() {
    let tmp = tempfile::tempdir().unwrap();
    let base = tmp.path().to_path_buf();
    fs::create_dir_all(&base).unwrap();

    // Step 1: Initialize
    recall_claude::init::run_with_base(&base).unwrap();

    assert!(base.join("rules/recall-claude.md").exists());
    assert!(base.join("memory/MEMORY.md").exists());
    assert!(base.join("EPHEMERAL.md").exists());
    assert!(base.join("ARCHIVE.md").exists());
    assert!(base.join("conversations").exists());
    assert!(base.join("settings.json").exists());

    // Step 2: Archive a session
    let jsonl_dir = tmp.path().join("transcripts");
    fs::create_dir_all(&jsonl_dir).unwrap();
    let transcript = write_synthetic_jsonl(&jsonl_dir, "session-001", "How do I refactor auth?");

    recall_claude::archive::archive_session_with_paths("session-001", &transcript, &base).unwrap();

    // Verify conversation file
    let conv_path = base.join("conversations/conversation-001.md");
    assert!(conv_path.exists());
    let conv_content = fs::read_to_string(&conv_path).unwrap();
    assert!(conv_content.contains("# Conversation 001"));
    assert!(conv_content.contains("### User"));
    assert!(conv_content.contains("How do I refactor auth?"));
    assert!(conv_content.contains("### Assistant"));
    assert!(conv_content.contains("Here is my response"));
    // Thinking blocks should NOT appear
    assert!(!conv_content.contains("Let me think about this"));

    // Verify ARCHIVE.md index
    let archive_content = fs::read_to_string(base.join("ARCHIVE.md")).unwrap();
    assert!(archive_content.contains("| 001 |"));
    assert!(archive_content.contains("session-001"));

    // Verify EPHEMERAL.md has entry
    let ephemeral_content = fs::read_to_string(base.join("EPHEMERAL.md")).unwrap();
    assert!(ephemeral_content.contains("Session session-001"));
    assert!(ephemeral_content.contains("conversation-001.md"));

    // Step 3: Archive a second session
    let transcript2 =
        write_synthetic_jsonl(&jsonl_dir, "session-002", "Can you fix the CI pipeline?");
    recall_claude::archive::archive_session_with_paths("session-002", &transcript2, &base).unwrap();

    assert!(base.join("conversations/conversation-002.md").exists());
    let ephemeral_content = fs::read_to_string(base.join("EPHEMERAL.md")).unwrap();
    assert!(ephemeral_content.contains("Session session-001"));
    assert!(ephemeral_content.contains("Session session-002"));

    // Step 4: Archive 5 more sessions to test EPHEMERAL FIFO trimming
    for i in 3..=7 {
        let transcript = write_synthetic_jsonl(
            &jsonl_dir,
            &format!("session-{i:03}"),
            &format!("Task number {i}"),
        );
        recall_claude::archive::archive_session_with_paths(
            &format!("session-{i:03}"),
            &transcript,
            &base,
        )
        .unwrap();
    }

    // Should have 7 conversation files
    let conv_count = fs::read_dir(base.join("conversations"))
        .unwrap()
        .filter(|e| {
            e.as_ref()
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with("conversation-")
        })
        .count();
    assert_eq!(conv_count, 7);

    // EPHEMERAL should be trimmed to 5
    let ephemeral_content = fs::read_to_string(base.join("EPHEMERAL.md")).unwrap();
    let entry_count = recall_claude::ephemeral::parse_entries(&ephemeral_content).len();
    assert_eq!(entry_count, 5);

    // Oldest two (session-001, session-002) should be trimmed
    assert!(!ephemeral_content.contains("Session session-001"));
    assert!(!ephemeral_content.contains("Session session-002"));
    // Most recent should still be there
    assert!(ephemeral_content.contains("Session session-007"));

    // Step 5: Status check should succeed
    assert!(recall_claude::status::run_with_base(&base).is_ok());
}

#[test]
fn empty_session_is_skipped() {
    let tmp = tempfile::tempdir().unwrap();
    let base = tmp.path().to_path_buf();
    fs::create_dir_all(&base).unwrap();
    recall_claude::init::run_with_base(&base).unwrap();

    // Create a JSONL with only queue-operations (no user messages)
    let jsonl_dir = tmp.path().join("transcripts");
    fs::create_dir_all(&jsonl_dir).unwrap();
    let path = jsonl_dir.join("empty.jsonl");
    let mut f = fs::File::create(&path).unwrap();
    writeln!(
        f,
        r#"{{"type":"queue-operation","operation":"enqueue","timestamp":"2026-03-05T14:30:00.000Z","sessionId":"empty"}}"#
    )
    .unwrap();

    recall_claude::archive::archive_session_with_paths("empty", &path.to_string_lossy(), &base)
        .unwrap();

    // No conversation file should be created
    let conv_count = fs::read_dir(base.join("conversations"))
        .unwrap()
        .filter(|e| {
            e.as_ref()
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with("conversation-")
        })
        .count();
    assert_eq!(conv_count, 0);
}
