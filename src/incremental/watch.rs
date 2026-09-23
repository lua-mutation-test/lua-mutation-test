//! File-system watching for incremental re-runs.

use notify::{Config as NotifyConfig, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

/// Watches `project_root` for file-system changes and invokes `on_change` after
/// events have settled for `debounce` duration.
///
/// The `should_stop` callback is checked between debounced events. Returning
/// `true` causes the watcher to exit cleanly.
pub fn watch_project<E>(
    project_root: &Path,
    debounce: Duration,
    mut on_change: impl FnMut() -> Result<(), E>,
    mut should_stop: impl FnMut() -> bool,
) -> Result<(), E>
where
    E: From<String>,
{
    let (tx, rx) = channel::<notify::Result<Event>>();
    let mut watcher = RecommendedWatcher::new(
        move |res: notify::Result<Event>| {
            let _ = tx.send(res);
        },
        NotifyConfig::default(),
    )
    .map_err(|e| E::from(e.to_string()))?;

    watcher
        .watch(project_root, RecursiveMode::Recursive)
        .map_err(|e| E::from(e.to_string()))?;

    while !should_stop() {
        if let Some(()) = debounced_event(&rx, debounce, &mut should_stop)? {
            on_change()?;
        }
    }

    Ok(())
}

/// Waits for a burst of events to settle.
///
/// Returns `Some(())` when an event was received and the debounce period elapsed
/// without further events. Returns `None` if `should_stop` became true while
/// waiting.
fn debounced_event<E>(
    rx: &Receiver<notify::Result<Event>>,
    debounce: Duration,
    should_stop: &mut impl FnMut() -> bool,
) -> Result<Option<()>, E>
where
    E: From<String>,
{
    // Wait for the first event.
    loop {
        if should_stop() {
            return Ok(None);
        }
        match rx.recv_timeout(Duration::from_millis(50)) {
            Ok(Ok(_)) => break,
            Ok(Err(e)) => return Err(E::from(e.to_string())),
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => {
                return Err(E::from("file watcher channel closed".to_string()))
            }
        }
    }

    // Drain subsequent events, resetting the timer on each one.
    let mut deadline = Instant::now() + debounce;
    while Instant::now() < deadline {
        if should_stop() {
            return Ok(None);
        }
        match rx.recv_timeout(Duration::from_millis(50).min(deadline - Instant::now())) {
            Ok(Ok(_)) => {
                deadline = Instant::now() + debounce;
            }
            Ok(Err(e)) => return Err(E::from(e.to_string())),
            Err(RecvTimeoutError::Timeout) => break,
            Err(RecvTimeoutError::Disconnected) => {
                return Err(E::from("file watcher channel closed".to_string()))
            }
        }
    }

    Ok(Some(()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    fn temp_dir() -> PathBuf {
        static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("lmt-watch-test-{}-{}", std::process::id(), n));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn debounce_coalesces_rapid_events() {
        let dir = temp_dir();
        let (tx, rx) = channel::<notify::Result<Event>>();
        let watcher_tx = tx.clone();
        let mut watcher = RecommendedWatcher::new(
            move |res| {
                let _ = watcher_tx.send(res);
            },
            NotifyConfig::default(),
        )
        .unwrap();
        watcher.watch(&dir, RecursiveMode::Recursive).unwrap();

        // Fire several events in rapid succession.
        for i in 0..3 {
            let file = dir.join(format!("file{}.txt", i));
            fs::write(&file, "x").unwrap();
        }

        let mut should_stop = || false;
        let result = debounced_event::<String>(&rx, Duration::from_millis(50), &mut should_stop)
            .unwrap()
            .is_some();
        assert!(result);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn watch_calls_on_change_when_file_changes() {
        let dir = temp_dir();
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = Arc::clone(&counter);
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let stop_clone = Arc::clone(&stop);

        // Use a background thread to modify a file after a short delay.
        let dir_clone = dir.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            fs::write(dir_clone.join("trigger.lua"), "x").unwrap();
        });

        let result = watch_project::<String>(
            &dir,
            Duration::from_millis(50),
            move || {
                counter_clone.fetch_add(1, Ordering::SeqCst);
                stop_clone.store(true, Ordering::SeqCst);
                Ok::<(), String>(())
            },
            || stop.load(Ordering::SeqCst),
        );

        assert!(result.is_ok());
        assert_eq!(counter.load(Ordering::SeqCst), 1);

        let _ = fs::remove_dir_all(&dir);
    }
}
