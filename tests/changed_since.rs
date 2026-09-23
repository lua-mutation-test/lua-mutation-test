//! End-to-end tests for `lmut run --changed-since <git-ref>`.
//!
//! Each test builds a small git repository in a temp dir (two source files,
//! two covering tests, real `lua` execution) and invokes the `lmut` binary.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn lmut_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_lmut"))
}

fn git_available() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn lua_available() -> bool {
    Command::new("lua")
        .arg("-v")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn run_git(dir: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("failed to run git");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn write_executable(path: &Path, contents: &str) {
    std::fs::write(path, contents).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(path).unwrap().permissions();
        perms.set_mode(perms.mode() | 0o111);
        std::fs::set_permissions(path, perms).unwrap();
    }
}

/// Builds a two-file Lua project with passing tests and commits it on `main`.
fn pr_fixture() -> tempfile::TempDir {
    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path();

    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(
        dir.join(".lua-mutation-test.toml"),
        "version = \"1\"\n\
         test_command = \"./run-tests.sh\"\n\
         source_globs = [\"src/*.lua\"]\n\
         test_globs = [\"test_*.lua\"]\n",
    )
    .unwrap();
    write_executable(
        &dir.join("run-tests.sh"),
        "#!/bin/sh\nset -e\ncd \"$(dirname \"$0\")\"\nlua test_a.lua\nlua test_b.lua\n",
    );
    std::fs::write(
        dir.join("src/a.lua"),
        "local function add(a, b)\n    return a + b\nend\n\nreturn {\n    add = add,\n}\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("src/b.lua"),
        "local function mul(a, b)\n    return a * b\nend\n\nreturn {\n    mul = mul,\n}\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("test_a.lua"),
        "local a = dofile(\"src/a.lua\")\n\nassert(a.add(2, 3) == 5)\nassert(a.add(-1, 1) == 0)\n\nprint(\"OK\")\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("test_b.lua"),
        "local b = dofile(\"src/b.lua\")\n\nassert(b.mul(2, 3) == 6)\nassert(b.mul(-1, 4) == -4)\n\nprint(\"OK\")\n",
    )
    .unwrap();

    run_git(dir, &["init", "-b", "main"]);
    run_git(dir, &["config", "user.email", "test@test.com"]);
    run_git(dir, &["config", "user.name", "Test"]);
    run_git(dir, &["add", "."]);
    run_git(dir, &["commit", "-m", "initial"]);

    temp
}

/// Runs the `lmut` binary in `dir` with the given extra args.
fn run_lmut(dir: &Path, args: &[&str]) -> Output {
    Command::new(lmut_bin())
        .arg("run")
        .arg(".")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("failed to run lmut")
}

fn stdout_text(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn stderr_text(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

#[test]
fn scoped_run_mutates_only_the_changed_file() {
    if !git_available() || !lua_available() {
        return;
    }
    let temp = pr_fixture();
    let dir = temp.path();

    // Simulate a PR touching only src/a.lua (comment-only change).
    run_git(dir, &["checkout", "-b", "feature"]);
    std::fs::write(
        dir.join("src/a.lua"),
        "local function add(a, b)\n    return a + b -- pr tweak\nend\n\nreturn {\n    add = add,\n}\n",
    )
    .unwrap();
    run_git(dir, &["add", "."]);
    run_git(dir, &["commit", "-m", "touch a"]);

    let output = run_lmut(
        dir,
        &[
            "--changed-since",
            "main",
            "--workers",
            "2",
            "--report-format",
            "json",
            "--report-output",
            "report.json",
        ],
    );
    let stderr = stderr_text(&output);
    assert!(
        output.status.success(),
        "scoped run should exit 0, got {:?}\nstderr:\n{stderr}",
        output.status.code()
    );
    assert!(
        stderr.contains("scoped to 1 file(s) changed since main"),
        "expected scope banner, stderr:\n{stderr}"
    );

    // The scoped JSON report must cover only the changed file's mutants.
    let report = std::fs::read_to_string(dir.join("report.json"))
        .expect("scoped JSON report should be written");
    let parsed: serde_json::Value =
        serde_json::from_str(&report).expect("report should be valid JSON");
    let results = parsed["results"]
        .as_array()
        .expect("report should contain a results array");
    assert!(
        !results.is_empty(),
        "scoped run should generate mutants for the changed file"
    );
    for result in results {
        let file = result["file"].as_str().unwrap_or_default();
        assert!(
            file.ends_with("src/a.lua"),
            "scoped report should only cover src/a.lua, found: {file}"
        );
    }
    let sources = parsed["metadata"]["source_paths"]
        .as_array()
        .expect("report should contain metadata.source_paths");
    assert_eq!(sources.len(), 1);
    assert!(sources[0]
        .as_str()
        .unwrap_or_default()
        .ends_with("src/a.lua"));

    // A second scoped run reuses the incremental cache: same mutants served
    // from cache with the existing `cached`/`ran` semantics.
    let output = run_lmut(dir, &["--changed-since", "main", "--workers", "2"]);
    let stdout = stdout_text(&output);
    assert!(
        output.status.success(),
        "second scoped run should exit 0\nstdout:\n{stdout}"
    );
    assert!(
        stdout.contains("cached 7, ran 0"),
        "second scoped run should serve all mutants from cache, stdout:\n{stdout}"
    );
}

#[test]
fn empty_scope_reports_zero_counts_and_exits_zero() {
    if !git_available() || !lua_available() {
        return;
    }
    let temp = pr_fixture();
    let dir = temp.path();

    // Clean tree on main: nothing differs from the base ref.
    let output = run_lmut(dir, &["--changed-since", "main", "--workers", "2"]);
    let stdout = stdout_text(&output);
    let stderr = stderr_text(&output);
    assert_eq!(
        output.status.code(),
        Some(0),
        "empty scope should exit 0\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stderr.contains("scoped to 0 file(s) changed since main"),
        "expected empty scope banner, stderr:\n{stderr}"
    );
    assert!(
        stdout.contains("Mutation score:"),
        "expected normal summary line, stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("survived 0"),
        "expected zero surviving mutants, stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("cached 0, ran 0"),
        "expected zero cached/ran counts, stdout:\n{stdout}"
    );
}

#[test]
fn unknown_ref_exits_two_and_names_the_ref() {
    if !git_available() || !lua_available() {
        return;
    }
    let temp = pr_fixture();
    let dir = temp.path();

    let output = run_lmut(dir, &["--changed-since", "does-not-exist"]);
    let stderr = stderr_text(&output);
    assert_eq!(
        output.status.code(),
        Some(2),
        "unknown ref should exit 2\nstderr:\n{stderr}"
    );
    assert!(
        stderr.contains("does-not-exist"),
        "error should name the bad ref, stderr:\n{stderr}"
    );
}

#[test]
fn changed_since_outside_git_tree_exits_two() {
    if !git_available() || !lua_available() {
        return;
    }
    // Plain Lua project with no git repository.
    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path();
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(
        dir.join(".lua-mutation-test.toml"),
        "version = \"1\"\n\
         test_command = \"./run-tests.sh\"\n\
         source_globs = [\"src/*.lua\"]\n\
         test_globs = [\"test_*.lua\"]\n",
    )
    .unwrap();
    write_executable(&dir.join("run-tests.sh"), "#!/bin/sh\nexit 0\n");
    std::fs::write(dir.join("src/a.lua"), "local x = 1\nreturn x\n").unwrap();
    std::fs::write(dir.join("test_a.lua"), "print(\"OK\")\n").unwrap();

    let output = run_lmut(dir, &["--changed-since", "main"]);
    let stderr = stderr_text(&output);
    assert_eq!(
        output.status.code(),
        Some(2),
        "non-git tree should exit 2\nstderr:\n{stderr}"
    );
    assert!(
        stderr.to_lowercase().contains("git"),
        "error should state git is required, stderr:\n{stderr}"
    );
}
