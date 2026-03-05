use std::fs;
use std::io::BufRead;
use std::path::Path;

use crate::paths;

const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const CYAN: &str = "\x1b[36m";
const YELLOW: &str = "\x1b[33m";
const RESET: &str = "\x1b[0m";

pub struct SearchResult {
    pub file: String,
    pub line_num: usize,
    pub line: String,
}

pub fn run(query: &str, context_lines: usize) -> Result<(), String> {
    let base = paths::claude_dir()?;
    let results = search_with_base(query, &base, context_lines)?;

    if results.is_empty() {
        eprintln!("No matches found for \"{query}\"");
        return Ok(());
    }

    eprintln!(
        "{BOLD}{} match{} across conversation archives{RESET}\n",
        results.len(),
        if results.len() == 1 { "" } else { "es" }
    );

    let mut current_file = String::new();
    for result in &results {
        if result.file != current_file {
            eprintln!("{CYAN}{}{RESET}", result.file);
            current_file = result.file.clone();
        }
        eprintln!("  {DIM}{:>4}{RESET}  {}", result.line_num, result.line);
    }

    Ok(())
}

pub fn search_with_base(
    query: &str,
    base: &Path,
    context_lines: usize,
) -> Result<Vec<SearchResult>, String> {
    let conversations_dir = base.join("conversations");
    if !conversations_dir.exists() {
        return Err(
            "conversations/ directory not found. Run `recall-claude init` first.".to_string(),
        );
    }

    let query_lower = query.to_lowercase();
    let mut results = Vec::new();

    // Collect and sort conversation files
    let mut files: Vec<_> = fs::read_dir(&conversations_dir)
        .map_err(|e| format!("Failed to read conversations directory: {e}"))?
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name();
            let name = name.to_string_lossy();
            name.starts_with("conversation-") && name.ends_with(".md")
        })
        .collect();
    files.sort_by_key(|e| e.file_name());

    for entry in &files {
        let file = std::io::BufReader::new(
            fs::File::open(entry.path())
                .map_err(|e| format!("Failed to open {}: {e}", entry.path().display()))?,
        );

        let lines: Vec<String> = file.lines().map_while(Result::ok).collect();
        let filename = entry.file_name().to_string_lossy().to_string();

        for (i, line) in lines.iter().enumerate() {
            if line.to_lowercase().contains(&query_lower) {
                // Add context lines before
                let start = i.saturating_sub(context_lines);
                for (ci, ctx_line) in lines.iter().enumerate().take(i).skip(start) {
                    results.push(SearchResult {
                        file: filename.clone(),
                        line_num: ci + 1,
                        line: format!("{DIM}{ctx_line}{RESET}"),
                    });
                }

                // The matching line (highlighted)
                let highlighted = highlight_match(line, query);
                results.push(SearchResult {
                    file: filename.clone(),
                    line_num: i + 1,
                    line: highlighted,
                });

                // Add context lines after
                let end = (i + context_lines + 1).min(lines.len());
                for (ci, ctx_line) in lines.iter().enumerate().take(end).skip(i + 1) {
                    results.push(SearchResult {
                        file: filename.clone(),
                        line_num: ci + 1,
                        line: format!("{DIM}{ctx_line}{RESET}"),
                    });
                }
            }
        }
    }

    Ok(results)
}

fn highlight_match(line: &str, query: &str) -> String {
    let lower_line = line.to_lowercase();
    let lower_query = query.to_lowercase();

    let mut result = String::new();
    let mut pos = 0;

    while let Some(found) = lower_line[pos..].find(&lower_query) {
        let abs_pos = pos + found;
        result.push_str(&line[pos..abs_pos]);
        result.push_str(YELLOW);
        result.push_str(BOLD);
        result.push_str(&line[abs_pos..abs_pos + query.len()]);
        result.push_str(RESET);
        pos = abs_pos + query.len();
    }
    result.push_str(&line[pos..]);

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_finds_matches() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();
        let conv_dir = base.join("conversations");
        fs::create_dir_all(&conv_dir).unwrap();

        fs::write(
            conv_dir.join("conversation-001.md"),
            "# Conversation 001\n\n### User\n\nHow do I refactor auth?\n\n### Assistant\n\nLet me check the auth module.\n",
        ).unwrap();

        let results = search_with_base("auth", base, 0).unwrap();
        assert_eq!(results.len(), 2); // "auth" appears in both user and assistant
    }

    #[test]
    fn search_case_insensitive() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();
        let conv_dir = base.join("conversations");
        fs::create_dir_all(&conv_dir).unwrap();

        fs::write(
            conv_dir.join("conversation-001.md"),
            "JWT tokens are great\n",
        )
        .unwrap();

        let results = search_with_base("jwt", base, 0).unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn search_no_matches() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();
        let conv_dir = base.join("conversations");
        fs::create_dir_all(&conv_dir).unwrap();

        fs::write(conv_dir.join("conversation-001.md"), "hello world\n").unwrap();

        let results = search_with_base("nonexistent", base, 0).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn search_with_context() {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path();
        let conv_dir = base.join("conversations");
        fs::create_dir_all(&conv_dir).unwrap();

        fs::write(
            conv_dir.join("conversation-001.md"),
            "line one\nline two\nfind this\nline four\nline five\n",
        )
        .unwrap();

        let results = search_with_base("find this", base, 1).unwrap();
        assert_eq!(results.len(), 3); // 1 before + match + 1 after
    }

    #[test]
    fn search_missing_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let result = search_with_base("test", tmp.path(), 0);
        assert!(result.is_err());
    }
}
