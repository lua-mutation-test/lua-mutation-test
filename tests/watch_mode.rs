use lua_mutation_test::config::Config;
use lua_mutation_test::incremental;
use lua_mutation_test::incremental::watch::watch_project;
use lua_mutation_test::mutant::MutantGenerator;
use lua_mutation_test::operators::default_operators;
use lua_mutation_test::parser::Parser as LuaParser;
use lua_mutation_test::runner::RunnerConfig;
use lua_mutation_test::test_discovery::discover_tests;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn write_file(path: &Path, contents: &str) {
    std::fs::write(path, contents).unwrap();
}

fn build_run(root: &Path) -> impl Fn() -> Result<(), String> {
    let root = root.to_path_buf();
    move || {
        let config = Config::default().with_test_command("exit 0");
        let tests = discover_tests(&root, &config.test_globs);
        if tests.is_empty() {
            return Err("no tests".to_string());
        }

        let source_files = vec![root.join("math.lua")];
        let source = std::fs::read_to_string(&source_files[0]).unwrap();
        let mut parser = LuaParser::new().map_err(|e| e.to_string())?;
        let tree = parser.parse_source(&source).map_err(|e| e.to_string())?;
        let generator = MutantGenerator::new(default_operators());
        let mutants: Vec<_> = generator
            .generate_validated(&source_files[0], &source, &tree)
            .0
            .into_iter()
            .map(|m| (m, source.clone()))
            .collect();

        let runner_config = RunnerConfig {
            command: vec!["sh".to_string(), "-c".to_string(), "exit 0".to_string()],
            timeout: Duration::from_secs(1),
            project_root: root.clone(),
            snippet_limit: 100,
        };

        let result = incremental::run_incremental(
            &root,
            &config,
            &runner_config,
            &source_files,
            mutants,
            1,
        )?;

        println!("ran {} mutants, cached {}", result.ran, result.cached);
        Ok(())
    }
}

#[test]
fn watch_mode_triggers_incremental_re_run_on_file_change() {
    let root = std::env::temp_dir().join(format!(
        "lmt-watch-integration-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();

    write_file(
        &root.join("math.lua"),
        "local M = {}\nfunction M.add(a, b)\n  return a + b\nend\nreturn M\n",
    );
    write_file(
        &root.join("math_spec.lua"),
        "describe('math', function() end)\n",
    );

    // Prime the cache with an initial run.
    build_run(&root)().unwrap();

    let run_count = Arc::new(AtomicUsize::new(0));
    let run_count_clone = Arc::clone(&run_count);
    let stop = Arc::new(Mutex::new(false));
    let stop_clone = Arc::clone(&stop);
    let stop_for_watch = Arc::clone(&stop);

    // Modify the source file after a short delay to trigger the watcher.
    let root_clone = root.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(150));
        write_file(
            &root_clone.join("math.lua"),
            "local M = {}\nfunction M.add(a, b)\n  return a - b\nend\nreturn M\n",
        );
    });

    let run = build_run(&root);
    let result = watch_project::<String>(
        &root,
        Duration::from_millis(50),
        move || {
            run_count_clone.fetch_add(1, Ordering::SeqCst);
            let result = run();
            *stop_clone.lock().unwrap() = true;
            result
        },
        || *stop_for_watch.lock().unwrap(),
    );

    assert!(result.is_ok(), "watch loop returned an error: {:?}", result);
    assert_eq!(run_count.load(Ordering::SeqCst), 1);

    let _ = std::fs::remove_dir_all(&root);
}
