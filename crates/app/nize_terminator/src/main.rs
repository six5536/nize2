// @awa-component: PLAN-005-Terminator
//! nize_terminator — process reaper for unclean shutdown cleanup.
//!
//! Watches a parent PID and executes cleanup commands from a manifest file
//! when the parent dies. Designed to survive SIGKILL of the parent process.

mod pid_watch;

use std::fs;
use std::path::PathBuf;
use std::process::{Command, ExitCode};

use clap::Parser;

/// Process reaper that watches a parent PID and runs cleanup commands on its death.
#[derive(Parser)]
#[command(name = "nize_terminator")]
struct Args {
    /// PID of the parent process to watch.
    #[arg(long)]
    parent_pid: u32,

    /// Path to the manifest file containing cleanup commands (one per line).
    #[arg(long)]
    manifest: PathBuf,
}

fn main() -> ExitCode {
    let args = Args::parse();

    // @awa-impl: PLAN-005 — wait for parent death
    pid_watch::wait_for_pid_exit(args.parent_pid);

    // @awa-impl: PLAN-005 — read manifest and execute cleanup commands
    let exit_code = run_cleanup(&args.manifest);

    // @awa-impl: PLAN-005 — delete manifest after cleanup
    if args.manifest.exists() {
        if let Err(e) = fs::remove_file(&args.manifest) {
            eprintln!("nize_terminator: failed to remove manifest: {e}");
        }
    }

    exit_code
}

/// Read the manifest file and execute each command via `sh -c`.
///
/// Returns `ExitCode::SUCCESS` if all commands succeed, `ExitCode::FAILURE` otherwise.
fn run_cleanup(manifest: &PathBuf) -> ExitCode {
    let contents = match fs::read_to_string(manifest) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("nize_terminator: failed to read manifest: {e}");
            return ExitCode::FAILURE;
        }
    };

    let commands = parse_manifest(&contents);

    if commands.is_empty() {
        return ExitCode::SUCCESS;
    }

    let mut all_ok = true;
    for cmd in &commands {
        eprintln!("nize_terminator: executing: {cmd}");
        // @awa-impl: PLAN-006-3.3
        #[cfg(unix)]
        let result = Command::new("sh").arg("-c").arg(cmd).status();
        #[cfg(windows)]
        let result = Command::new("cmd").arg("/C").arg(cmd).status();

        match result {
            Ok(status) if status.success() => {}
            Ok(status) => {
                eprintln!(
                    "nize_terminator: command exited with {}: {cmd}",
                    status.code().unwrap_or(-1)
                );
                all_ok = false;
            }
            Err(e) => {
                eprintln!("nize_terminator: failed to execute command: {e}");
                all_ok = false;
            }
        }
    }

    if all_ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// Parse a manifest file's contents into a list of commands.
///
/// Skips blank lines and lines starting with `#` (comments).
fn parse_manifest(contents: &str) -> Vec<&str> {
    contents
        .lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // @awa-test: PLAN-005-ManifestParsing
    #[test]
    fn parse_manifest_skips_blanks_and_comments() {
        let input = "\
pg_ctl -D /data -m fast stop

# this is a comment
kill 12345


";
        let commands = parse_manifest(input);
        assert_eq!(commands, vec!["pg_ctl -D /data -m fast stop", "kill 12345"]);
    }

    // @awa-test: PLAN-005-ManifestParsing
    #[test]
    fn parse_manifest_empty_input() {
        let commands = parse_manifest("");
        assert!(commands.is_empty());
    }

    // @awa-test: PLAN-005-ManifestParsing
    #[test]
    fn parse_manifest_trims_whitespace() {
        let input = "  pg_ctl stop  \n  kill 1  ";
        let commands = parse_manifest(input);
        assert_eq!(commands, vec!["pg_ctl stop", "kill 1"]);
    }

    // @awa-test: PLAN-005-CleanupExecution
    #[test]
    fn run_cleanup_with_successful_commands() {
        let dir = tempfile::tempdir().expect("tempdir");
        let manifest = dir.path().join("cleanup.manifest");
        fs::write(&manifest, "true\ntrue\n").expect("write manifest");
        let code = run_cleanup(&manifest);
        assert_eq!(code, ExitCode::SUCCESS);
    }

    // @awa-test: PLAN-005-CleanupExecution
    #[test]
    fn run_cleanup_with_failing_command() {
        let dir = tempfile::tempdir().expect("tempdir");
        let manifest = dir.path().join("cleanup.manifest");
        fs::write(&manifest, "true\nfalse\ntrue\n").expect("write manifest");
        let code = run_cleanup(&manifest);
        assert_eq!(code, ExitCode::FAILURE);
    }

    // @awa-test: PLAN-005-CleanupExecution
    #[test]
    fn run_cleanup_nonexistent_manifest() {
        let path = PathBuf::from("/nonexistent/cleanup.manifest");
        let code = run_cleanup(&path);
        assert_eq!(code, ExitCode::FAILURE);
    }

    // @awa-test: PLAN-005-CleanupExecution
    #[test]
    fn run_cleanup_empty_manifest() {
        let dir = tempfile::tempdir().expect("tempdir");
        let manifest = dir.path().join("cleanup.manifest");
        fs::write(&manifest, "\n\n# just comments\n").expect("write manifest");
        let code = run_cleanup(&manifest);
        assert_eq!(code, ExitCode::SUCCESS);
    }
}
