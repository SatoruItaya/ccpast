use std::path::Path;
use std::process::{Command, ExitStatus};

#[derive(Debug)]
pub enum ResumeError {
    CwdMissing,
    ClaudeNotFound,
    SpawnFailed(String),
    NonZeroExit(i32),
}

impl std::fmt::Display for ResumeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CwdMissing => write!(f, "the original working directory no longer exists"),
            Self::ClaudeNotFound => write!(f, "`claude` not found on PATH"),
            Self::SpawnFailed(s) => write!(f, "failed to spawn `claude`: {s}"),
            Self::NonZeroExit(code) => write!(f, "`claude` exited with status {code}"),
        }
    }
}

/// Spawn `claude --resume <id>` with the child's `current_dir` set to `cwd`.
/// stdio is inherited so the child takes over the terminal. Returns when the
/// child exits.
pub fn spawn(
    cwd: &Path,
    config_dir: &Path,
    session_id: &str,
    fork: bool,
) -> Result<(), ResumeError> {
    if !cwd.is_dir() {
        return Err(ResumeError::CwdMissing);
    }
    let mut cmd = Command::new("claude");
    cmd.current_dir(cwd).arg("--resume").arg(session_id);
    // Setting CLAUDE_CONFIG_DIR, even to ~/.claude, also moves Claude Code's
    // global config file, so only set it for non-default dirs.
    if crate::scan::default_config_dir().as_deref() == Some(config_dir) {
        cmd.env_remove("CLAUDE_CONFIG_DIR");
    } else {
        cmd.env("CLAUDE_CONFIG_DIR", config_dir);
    }
    if fork {
        cmd.arg("--fork-session");
    }
    let status: ExitStatus = match cmd.status() {
        Ok(s) => s,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Err(ResumeError::ClaudeNotFound);
        }
        Err(err) => return Err(ResumeError::SpawnFailed(err.to_string())),
    };
    if !status.success() {
        return Err(ResumeError::NonZeroExit(status.code().unwrap_or(-1)));
    }
    Ok(())
}
