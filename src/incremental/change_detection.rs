//! Detects source files that have changed since the last mutation run.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Returns the set of source files that have changed.
///
/// When the project is inside a git repository, git is used to detect modified
/// files and commits that differ from the cached HEAD. Otherwise the cached
/// file hashes are compared against the current file contents.
pub fn changed_files(
    project_root: &Path,
    cached_hashes: &HashMap<String, String>,
    source_files: &[PathBuf],
    cached_git_head: Option<&str>,
) -> Result<Vec<PathBuf>, String> {
    if let Some(files) = git_changed_files(project_root, cached_git_head)? {
        return Ok(files);
    }
    Ok(hash_based_changed_files(cached_hashes, source_files))
}

/// Uses git to detect changed files. Returns `Ok(None)` when git is unavailable.
fn git_changed_files(
    project_root: &Path,
    cached_git_head: Option<&str>,
) -> Result<Option<Vec<PathBuf>>, String> {
    let inside = match Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(project_root)
        .output()
    {
        Ok(output) => output,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };

    if !inside.status.success() {
        return Ok(None);
    }

    let mut files = Vec::new();

    // Files modified in the working tree.
    let status = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(project_root)
        .output()
        .map_err(|e| e.to_string())?;
    if status.status.success() {
        let stdout = String::from_utf8_lossy(&status.stdout);
        for line in stdout.lines() {
            if line.len() >= 3 {
                files.push(project_root.join(&line[3..]));
            }
        }
    }

    // Files changed between the cached commit and the current HEAD.
    if let Some(cached) = cached_git_head {
        let head = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(project_root)
            .output()
            .map_err(|e| e.to_string())?;
        if head.status.success() {
            let current_head = String::from_utf8_lossy(&head.stdout);
            let current_head = current_head.trim();
            if current_head != cached {
                let diff = Command::new("git")
                    .args(["diff", "--name-only", cached, current_head])
                    .current_dir(project_root)
                    .output()
                    .map_err(|e| e.to_string())?;
                if diff.status.success() {
                    let stdout = String::from_utf8_lossy(&diff.stdout);
                    for line in stdout.lines() {
                        files.push(project_root.join(line));
                    }
                }
            }
        }
    }

    files.sort();
    files.dedup();
    Ok(Some(files))
}

/// Returns the files differing from a base git ref.
///
/// Resolves `base_ref` with `git rev-parse --verify`, diffs
/// `merge-base(HEAD, ref)..HEAD` with `git diff --name-only`, and adds
/// uncommitted working-tree changes from `git status --porcelain`.
/// Returned paths are absolute (canonicalized where the filesystem allows),
/// sorted, and deduplicated.
///
/// Errors when `project_root` is not inside a git work tree or when the ref
/// cannot be resolved, with the ref named in the message.
pub fn changed_since_files(project_root: &Path, base_ref: &str) -> Result<Vec<PathBuf>, String> {
    // Canonicalize once so diff output (joined onto the repo root) and status
    // output (joined onto the working directory) compare equal.
    let project_root =
        std::fs::canonicalize(project_root).unwrap_or_else(|_| project_root.to_path_buf());

    let run_git = |args: &[&str]| -> Result<std::process::Output, String> {
        Command::new("git")
            .args(args)
            .current_dir(&project_root)
            .output()
            .map_err(|e| format!("failed to run git: {e}"))
    };
    let stdout_lines = |output: &std::process::Output| -> Vec<String> {
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(|line| line.to_string())
            .collect()
    };

    let inside = run_git(&["rev-parse", "--is-inside-work-tree"])?;
    if !inside.status.success() {
        return Err(format!(
            "--changed-since {base_ref}: '{}' is not inside a git work tree; changed-files mode requires git",
            project_root.display()
        ));
    }

    if !run_git(&["rev-parse", "--verify", base_ref])?
        .status
        .success()
    {
        return Err(format!(
            "--changed-since: unknown or unresolvable git ref '{base_ref}'"
        ));
    }
    let merge_base_out = run_git(&["merge-base", "HEAD", base_ref])?;
    if !merge_base_out.status.success() {
        return Err(format!(
            "--changed-since: cannot compute merge base of HEAD with git ref '{base_ref}'"
        ));
    }
    let merge_base = stdout_lines(&merge_base_out)
        .into_iter()
        .next()
        .unwrap_or_default();

    // `git diff --name-only` reports paths relative to the repository root,
    // while `git status --porcelain` reports them relative to the current
    // directory, so each is joined onto the matching base. The leading
    // two-column status prefix must be preserved (no trimming).
    let toplevel_out = run_git(&["rev-parse", "--show-toplevel"])?;
    let repo_root = stdout_lines(&toplevel_out)
        .into_iter()
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| project_root.clone());

    let mut files = Vec::new();
    let diff_out = run_git(&["diff", "--name-only", &merge_base, "HEAD"])?;
    if diff_out.status.success() {
        for line in stdout_lines(&diff_out) {
            if !line.is_empty() {
                files.push(repo_root.join(line));
            }
        }
    }
    let status_out = run_git(&["status", "--porcelain"])?;
    if status_out.status.success() {
        for line in stdout_lines(&status_out) {
            if line.len() >= 3 {
                files.push(project_root.join(&line[3..]));
            }
        }
    }

    files.sort();
    files.dedup();
    Ok(files)
}

/// Fallback change detection based on SHA-256 file hashes.
fn hash_based_changed_files(
    cached_hashes: &HashMap<String, String>,
    source_files: &[PathBuf],
) -> Vec<PathBuf> {
    use super::cache::file_hash;

    let mut changed = Vec::new();
    for path in source_files {
        let key = path.to_string_lossy().to_string();
        let current = file_hash(path).ok();
        let previous = cached_hashes.get(&key);
        if current.as_ref() != previous {
            changed.push(path.clone());
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::incremental::cache::file_hash;
    use std::fs;

    fn temp_project() -> PathBuf {
        static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let dir =
            std::env::temp_dir().join(format!("lmt-detect-test-{}-{}", std::process::id(), n));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn hash_based_detects_modified_file() {
        let dir = temp_project();
        let file = dir.join("foo.lua");
        fs::write(&file, "original").unwrap();

        let mut cached = HashMap::new();
        cached.insert(
            file.to_string_lossy().to_string(),
            file_hash(&file).unwrap(),
        );

        fs::write(&file, "modified").unwrap();
        let changed = hash_based_changed_files(&cached, std::slice::from_ref(&file));
        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0], file);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn hash_based_ignores_unchanged_file() {
        let dir = temp_project();
        let file = dir.join("foo.lua");
        fs::write(&file, "same").unwrap();

        let mut cached = HashMap::new();
        cached.insert(
            file.to_string_lossy().to_string(),
            file_hash(&file).unwrap(),
        );

        let changed = hash_based_changed_files(&cached, std::slice::from_ref(&file));
        assert!(changed.is_empty());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn git_detects_modified_file() {
        if !git_available() {
            return;
        }

        let dir = temp_project();
        let file = dir.join("foo.lua");
        fs::write(&file, "original").unwrap();

        // Initialize a git repo and make an initial commit.
        run_git(&dir, &["init"]).unwrap();
        run_git(&dir, &["config", "user.email", "test@test.com"]).unwrap();
        run_git(&dir, &["config", "user.name", "Test"]).unwrap();
        run_git(&dir, &["add", "."]).unwrap();
        run_git(&dir, &["commit", "-m", "initial"]).unwrap();

        // No changes yet.
        let changed = git_changed_files(&dir, None).unwrap().unwrap();
        assert!(changed.is_empty());

        // Modify a file.
        fs::write(&file, "modified").unwrap();
        let changed = git_changed_files(&dir, None).unwrap().unwrap();
        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0], file);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn non_git_project_returns_none() {
        if !git_available() {
            return;
        }

        let dir = temp_project();
        assert!(git_changed_files(&dir, None).unwrap().is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    fn init_repo_on_main(dir: &Path) {
        run_git(dir, &["init", "-b", "main"]).unwrap();
        run_git(dir, &["config", "user.email", "test@test.com"]).unwrap();
        run_git(dir, &["config", "user.name", "Test"]).unwrap();
    }

    fn commit_all(dir: &Path, message: &str) {
        run_git(dir, &["add", "."]).unwrap();
        run_git(dir, &["commit", "-m", message]).unwrap();
    }

    fn canonical(dir: &Path) -> PathBuf {
        fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf())
    }

    #[test]
    fn changed_since_returns_files_changed_on_branch() {
        if !git_available() {
            return;
        }

        let dir = temp_project();
        let a = dir.join("a.lua");
        let b = dir.join("b.lua");
        fs::write(&a, "local x = 1").unwrap();
        fs::write(&b, "local y = 2").unwrap();
        init_repo_on_main(&dir);
        commit_all(&dir, "initial");

        run_git(&dir, &["checkout", "-b", "feature"]).unwrap();
        fs::write(&a, "local x = 10").unwrap();
        commit_all(&dir, "change a");

        let changed = changed_since_files(&dir, "main").unwrap();
        assert_eq!(changed, vec![canonical(&dir).join("a.lua")]);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn changed_since_excludes_unchanged_files() {
        if !git_available() {
            return;
        }

        let dir = temp_project();
        let a = dir.join("a.lua");
        let b = dir.join("b.lua");
        fs::write(&a, "local x = 1").unwrap();
        fs::write(&b, "local y = 2").unwrap();
        init_repo_on_main(&dir);
        commit_all(&dir, "initial");

        // Clean tree on the same ref: nothing differs.
        let changed = changed_since_files(&dir, "main").unwrap();
        assert!(changed.is_empty());
        assert!(!changed.contains(&a));
        assert!(!changed.contains(&b));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn changed_since_includes_uncommitted_changes() {
        if !git_available() {
            return;
        }

        let dir = temp_project();
        let a = dir.join("a.lua");
        fs::write(&a, "local x = 1").unwrap();
        init_repo_on_main(&dir);
        commit_all(&dir, "initial");

        // Modify without committing.
        fs::write(&a, "local x = 99").unwrap();

        let changed = changed_since_files(&dir, "main").unwrap();
        assert_eq!(changed, vec![canonical(&dir).join("a.lua")]);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn changed_since_unknown_ref_errors_naming_ref() {
        if !git_available() {
            return;
        }

        let dir = temp_project();
        fs::write(dir.join("a.lua"), "local x = 1").unwrap();
        init_repo_on_main(&dir);
        commit_all(&dir, "initial");

        let err = changed_since_files(&dir, "does-not-exist").unwrap_err();
        assert!(
            err.contains("does-not-exist"),
            "error should name the bad ref: {err}"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn changed_since_non_git_dir_errors() {
        if !git_available() {
            return;
        }

        let dir = temp_project();
        let err = changed_since_files(&dir, "main").unwrap_err();
        assert!(
            err.contains("git"),
            "error should state git is required: {err}"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    fn git_available() -> bool {
        Command::new("git")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn run_git(dir: &Path, args: &[&str]) -> Result<(), String> {
        let output = Command::new("git")
            .args(args)
            .current_dir(dir)
            .output()
            .map_err(|e| e.to_string())?;
        if output.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }
}
