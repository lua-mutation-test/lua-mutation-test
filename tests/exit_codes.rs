use std::path::{Path, PathBuf};
use std::process::Command;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> std::io::Result<()> {
    std::fs::create_dir_all(&dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(entry.path(), dst.as_ref().join(entry.file_name()))?;
        } else {
            std::fs::copy(entry.path(), dst.as_ref().join(entry.file_name()))?;
        }
    }
    Ok(())
}

fn make_executable(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(path).unwrap().permissions();
        perms.set_mode(perms.mode() | 0o111);
        std::fs::set_permissions(path, perms).unwrap();
    }
}

fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_lua-mutation-test"))
}

#[test]
fn red_baseline_exits_3() {
    let temp = tempfile::tempdir().unwrap();
    copy_dir_all(fixture_path("failing_baseline_project"), temp.path()).unwrap();
    let runner = temp.path().join("busted");
    make_executable(&runner);

    let output = Command::new(binary())
        .args([
            "run",
            temp.path().to_str().unwrap(),
            "--test-command",
            runner.to_str().unwrap(),
        ])
        .output()
        .expect("failed to run binary");
    assert_eq!(
        output.status.code(),
        Some(3),
        "red baseline should exit 3, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn survivors_exit_1() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(
        temp.path().join("math.lua"),
        "local M = {}\nfunction M.add(a, b)\n  return a + b\nend\nreturn M\n",
    )
    .unwrap();
    std::fs::write(
        temp.path().join("math_spec.lua"),
        "describe('math', function() end)\n",
    )
    .unwrap();
    let runner = temp.path().join("run-tests.sh");
    std::fs::write(&runner, "#!/bin/sh\nexit 0\n").unwrap();
    make_executable(&runner);

    let output = Command::new(binary())
        .args([
            "run",
            temp.path().to_str().unwrap(),
            "--test-command",
            runner.to_str().unwrap(),
        ])
        .output()
        .expect("failed to run binary");
    assert_eq!(
        output.status.code(),
        Some(1),
        "surviving mutants should exit 1, stdout: {} stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn missing_test_command_exits_2() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(temp.path().join("a.lua"), "local x = 1\n").unwrap();
    std::fs::write(
        temp.path().join("a_spec.lua"),
        "describe('a', function() end)\n",
    )
    .unwrap();

    let output = Command::new(binary())
        .args(["run", temp.path().to_str().unwrap()])
        .output()
        .expect("failed to run binary");
    assert_eq!(
        output.status.code(),
        Some(2),
        "missing test command should exit 2, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
