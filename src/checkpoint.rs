use std::fs;
use std::path::Path;

use crate::archive;
use crate::frontmatter::Frontmatter;
use crate::jsonl;
use crate::paths;
use crate::tags;

struct CheckpointData {
    session_id: String,
    topics: Vec<String>,
    message_count: u32,
    duration: String,
    md_body: String,
    tags_section: String,
}

/// Checkpoint command — called by the PreCompact hook.
/// Reads hook input from stdin to get transcript_path, parses the partial
/// transcript, extracts topics/metadata, and writes a checkpoint archive.
pub fn run(trigger: &str) -> Result<(), String> {
    run_with_paths(trigger, &paths::claude_dir()?)
}

pub fn run_with_paths(trigger: &str, base_dir: &Path) -> Result<(), String> {
    let conversations_dir = base_dir.join("conversations");
    let archive_index = base_dir.join("ARCHIVE.md");

    if !conversations_dir.exists() {
        return Err(
            "conversations/ directory not found. Run `recall-claude init` first.".to_string(),
        );
    }

    // Try to read hook input from stdin (Claude Code passes transcript_path)
    let hook_input = jsonl::read_hook_input().ok();

    let next_num = archive::highest_conversation_number(&conversations_dir) + 1;
    let now = jsonl::utc_now();
    let date = jsonl::date_from_timestamp(&now);

    // If we have hook input with a transcript, parse it for metadata
    let data = match &hook_input {
        Some(input) => extract_from_transcript(input).unwrap_or_else(empty_checkpoint),
        None => empty_checkpoint(),
    };

    let fm = Frontmatter {
        log: next_num,
        date: now,
        session_id: data.session_id,
        message_count: data.message_count,
        duration: data.duration.clone(),
        source: trigger.to_string(),
        topics: data.topics.clone(),
    };

    let full_content = format!("{}\n\n{}{}", fm.render(), data.md_body, data.tags_section);

    // Write conversation file
    let conv_file = conversations_dir.join(format!("conversation-{next_num:03}.md"));
    fs::write(&conv_file, &full_content)
        .map_err(|e| format!("Failed to write checkpoint file: {e}"))?;

    // Append to ARCHIVE.md
    archive::append_index(
        &archive_index,
        next_num,
        &date,
        &fm.session_id,
        &data.topics,
        data.message_count,
        &data.duration,
    )?;

    eprintln!(
        "recall-claude: checkpoint conversation-{:03}.md ({} — {} messages, {} topics)",
        next_num,
        trigger,
        data.message_count,
        data.topics.len()
    );

    Ok(())
}

/// Extract metadata from a partial transcript via hook input.
fn extract_from_transcript(input: &jsonl::HookInput) -> Option<CheckpointData> {
    let conv = jsonl::parse_transcript(&input.transcript_path, &input.session_id).ok()?;

    if conv.user_message_count == 0 {
        return None;
    }

    let duration = match (&conv.first_timestamp, &conv.last_timestamp) {
        (Some(first), Some(last)) => jsonl::calculate_duration(first, last),
        _ => "unknown".to_string(),
    };
    let total_messages = conv.user_message_count + conv.assistant_message_count;
    let topics = jsonl::extract_topics(&conv, 5);
    let md_body = jsonl::conversation_to_markdown(&conv, 0);
    let conv_tags = tags::extract_tags(&conv);
    let tags_section = tags::format_tags_section(&conv_tags);

    Some(CheckpointData {
        session_id: input.session_id.clone(),
        topics,
        message_count: total_messages,
        duration,
        md_body,
        tags_section,
    })
}

/// Fallback when no transcript is available (manual invocation).
fn empty_checkpoint() -> CheckpointData {
    CheckpointData {
        session_id: String::new(),
        topics: vec![],
        message_count: 0,
        duration: String::new(),
        md_body: "# Checkpoint\n\nNo transcript available.\n".to_string(),
        tags_section: String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_test_jsonl(dir: &Path) -> String {
        let path = dir.join("test-session.jsonl");
        let mut f = fs::File::create(&path).unwrap();
        let lines = [
            r#"{"type":"queue-operation","operation":"enqueue","timestamp":"2026-03-05T14:30:00.000Z","sessionId":"test-ckpt"}"#,
            r#"{"parentUuid":null,"type":"user","sessionId":"test-ckpt","timestamp":"2026-03-05T14:30:00.100Z","message":{"role":"user","content":"Let's refactor the auth module to use JWT"}}"#,
            r#"{"parentUuid":"aaa","type":"assistant","sessionId":"test-ckpt","timestamp":"2026-03-05T14:30:05.000Z","message":{"role":"assistant","content":[{"type":"text","text":"I'll refactor the auth module to use JWT tokens."}]}}"#,
            r#"{"parentUuid":"bbb","type":"assistant","sessionId":"test-ckpt","timestamp":"2026-03-05T14:30:06.000Z","message":{"role":"assistant","content":[{"type":"tool_use","id":"toolu_abc","name":"Read","input":{"file_path":"/src/auth.rs"}}]}}"#,
            r#"{"parentUuid":"ccc","type":"user","sessionId":"test-ckpt","timestamp":"2026-03-05T14:30:07.000Z","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"toolu_abc","content":"pub fn login() {}"}]}}"#,
            r#"{"parentUuid":"ddd","type":"user","sessionId":"test-ckpt","timestamp":"2026-03-05T14:35:00.000Z","message":{"role":"user","content":"Now add token validation"}}"#,
            r#"{"parentUuid":"eee","type":"assistant","sessionId":"test-ckpt","timestamp":"2026-03-05T14:35:05.000Z","message":{"role":"assistant","content":[{"type":"text","text":"Adding token validation now."}]}}"#,
        ];
        for line in &lines {
            writeln!(f, "{}", line).unwrap();
        }
        path.to_string_lossy().to_string()
    }

    #[test]
    fn checkpoint_with_transcript_extracts_topics() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();
        let conv_dir = base.join("conversations");
        fs::create_dir_all(&conv_dir).unwrap();
        fs::write(base.join("ARCHIVE.md"), "").unwrap();

        let transcript = write_test_jsonl(tmp.path());
        let input = jsonl::HookInput {
            session_id: "test-ckpt".to_string(),
            transcript_path: transcript,
            cwd: None,
            hook_event_name: Some("PreCompact".to_string()),
        };

        let data = extract_from_transcript(&input);
        assert!(data.is_some());

        let data = data.unwrap();
        assert_eq!(data.session_id, "test-ckpt");
        assert!(data.message_count > 0);
        assert!(!data.topics.is_empty());
        assert!(
            data.md_body.contains("auth")
                || data.md_body.contains("JWT")
                || data.md_body.contains("refactor")
        );
    }

    #[test]
    fn checkpoint_writes_archive_entry_with_topics() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();
        let conv_dir = base.join("conversations");
        fs::create_dir_all(&conv_dir).unwrap();

        let archive_path = base.join("ARCHIVE.md");
        fs::write(&archive_path, "").unwrap();

        let transcript = write_test_jsonl(tmp.path());

        // Simulate what run_with_paths does with a known transcript
        let input = jsonl::HookInput {
            session_id: "test-ckpt".to_string(),
            transcript_path: transcript,
            cwd: None,
            hook_event_name: Some("PreCompact".to_string()),
        };

        let data = extract_from_transcript(&input).unwrap();

        let fm = Frontmatter {
            log: 1,
            date: "2026-03-05T14:30:00Z".to_string(),
            session_id: data.session_id.clone(),
            message_count: data.message_count,
            duration: data.duration.clone(),
            source: "precompact".to_string(),
            topics: data.topics.clone(),
        };

        let conv_file = conv_dir.join("conversation-001.md");
        fs::write(
            &conv_file,
            format!("{}\n\n{}{}", fm.render(), data.md_body, data.tags_section),
        )
        .unwrap();

        archive::append_index(
            &archive_path,
            1,
            "2026-03-05",
            &data.session_id,
            &data.topics,
            data.message_count,
            &data.duration,
        )
        .unwrap();

        // Verify ARCHIVE.md has topics
        let archive_content = fs::read_to_string(&archive_path).unwrap();
        assert!(
            !archive_content.contains("| — |"),
            "Topics should not be empty dash"
        );

        // Verify conversation file has frontmatter with topics
        let conv_content = fs::read_to_string(&conv_file).unwrap();
        assert!(conv_content.contains("topics:"));
        assert!(!conv_content.contains("topics: []"));
    }

    #[test]
    fn empty_checkpoint_fallback() {
        let data = empty_checkpoint();
        assert!(data.session_id.is_empty());
        assert!(data.topics.is_empty());
        assert_eq!(data.message_count, 0);
        assert!(data.duration.is_empty());
        assert!(data.md_body.contains("No transcript available"));
        assert!(data.tags_section.is_empty());
    }
}
