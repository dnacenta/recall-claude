use std::path::PathBuf;

/// Returns ~/.claude or RECALL_CLAUDE_HOME override.
pub fn claude_dir() -> Result<PathBuf, String> {
    if let Ok(p) = std::env::var("RECALL_CLAUDE_HOME") {
        return Ok(PathBuf::from(p));
    }
    let home = dirs::home_dir().ok_or("Could not determine home directory")?;
    Ok(home.join(".claude"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_override_takes_precedence() {
        unsafe { std::env::set_var("RECALL_CLAUDE_HOME", "/tmp/test-claude") };
        let dir = claude_dir().unwrap();
        assert_eq!(dir, PathBuf::from("/tmp/test-claude"));
        unsafe { std::env::remove_var("RECALL_CLAUDE_HOME") };
    }

    #[test]
    fn default_returns_home_dot_claude() {
        unsafe { std::env::remove_var("RECALL_CLAUDE_HOME") };
        let dir = claude_dir().unwrap();
        let home = dirs::home_dir().unwrap();
        assert_eq!(dir, home.join(".claude"));
    }
}
