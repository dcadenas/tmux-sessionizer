use std::collections::HashSet;
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};

use tms::tmux::Tmux;

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// A test tmux server on an isolated socket.
/// Automatically killed on drop.
pub struct TmuxHarness {
    pub socket: String,
    pub tmux: Tmux,
}

impl Default for TmuxHarness {
    fn default() -> Self {
        Self::new()
    }
}

impl TmuxHarness {
    pub fn new() -> Self {
        let id = COUNTER.fetch_add(1, Ordering::SeqCst);
        let socket = format!("tms-test-{}-{}", std::process::id(), id);
        let tmux = Tmux::with_socket(&socket);

        // Start the server by creating an initial detached session
        let output = Command::new("tmux")
            .args(["-L", &socket, "new-session", "-d", "-s", "init"])
            .output()
            .expect("failed to start test tmux server");
        assert!(output.status.success(), "failed to create test tmux server");

        Self { socket, tmux }
    }

    /// Create a session with optional windows.
    /// Returns the session name.
    pub fn create_session(&self, name: &str, windows: &[&str]) -> String {
        self.tmux.new_session(Some(name), None);

        for win_name in windows {
            self.tmux.new_window(Some(win_name), None, Some(name));
        }

        name.to_string()
    }

    /// Raw tmux command on this socket.
    pub fn run_tmux(&self, args: &[&str]) -> String {
        let output = Command::new("tmux")
            .args(["-L", &self.socket])
            .args(args)
            .output()
            .expect("failed to run tmux command");
        String::from_utf8(output.stdout).unwrap()
    }
}

impl Drop for TmuxHarness {
    fn drop(&mut self) {
        let _ = Command::new("tmux")
            .args(["-L", &self.socket, "kill-server"])
            .output();
    }
}

// --- Tests ---

#[test]
fn test_session_exists() {
    let h = TmuxHarness::new();
    h.create_session("my-project", &[]);

    assert!(h.tmux.session_exists("my-project"));
    assert!(!h.tmux.session_exists("nonexistent"));
}

#[test]
fn test_list_sessions() {
    let h = TmuxHarness::new();
    h.create_session("alpha", &[]);
    h.create_session("beta", &[]);

    let sessions = h.tmux.list_sessions("#S");
    let names: HashSet<&str> = sessions.trim().lines().collect();

    assert!(names.contains("alpha"));
    assert!(names.contains("beta"));
    assert!(names.contains("init")); // from harness setup
}

#[test]
fn test_list_session_windows() {
    let h = TmuxHarness::new();
    h.create_session("multi", &["feature-a", "feature-b"]);

    let windows = h.tmux.list_session_windows("multi");

    // Should have 3 windows: the default one + 2 we created
    assert!(
        windows.len() >= 3,
        "expected at least 3 windows, got {}: {:?}",
        windows.len(),
        windows
    );

    // Each entry should be "index:display_name"
    for win in &windows {
        assert!(
            win.contains(':'),
            "window entry should contain ':' separator, got: {}",
            win
        );
    }
}

#[test]
fn test_expand_windows_single_window_not_expanded() {
    let h = TmuxHarness::new();
    h.create_session("solo", &[]);

    let active: HashSet<&str> = ["solo"].into();
    let result = tms::expand_windows(vec!["solo".to_string()], &active, &h.tmux);

    assert_eq!(result, vec!["solo"]);
}

#[test]
fn test_expand_windows_multi_window_expanded() {
    let h = TmuxHarness::new();
    h.create_session("multi", &["win-a", "win-b"]);

    let active: HashSet<&str> = ["multi"].into();
    let result = tms::expand_windows(vec!["multi".to_string()], &active, &h.tmux);

    // Should be expanded into multi/... entries
    assert!(
        result.len() >= 3,
        "expected at least 3 entries, got {}: {:?}",
        result.len(),
        result
    );
    for entry in &result {
        assert!(
            entry.starts_with("multi/"),
            "expanded entry should start with 'multi/', got: {}",
            entry
        );
    }
}

#[test]
fn test_expand_windows_inactive_not_expanded() {
    let h = TmuxHarness::new();
    h.create_session("active-one", &["extra"]);

    // "inactive-repo" is not in the active set, should stay as-is
    let active: HashSet<&str> = ["active-one"].into();
    let result = tms::expand_windows(
        vec!["active-one".to_string(), "inactive-repo".to_string()],
        &active,
        &h.tmux,
    );

    assert!(
        result.contains(&"inactive-repo".to_string()),
        "inactive session should remain unexpanded: {:?}",
        result
    );
}

#[test]
fn test_expand_windows_normalized_name_match() {
    let h = TmuxHarness::new();
    // tmux session is "my_project" (underscores), repo name is "my-project" (hyphens)
    h.create_session("my_project", &["branch-a"]);

    let active: HashSet<&str> = ["my_project"].into();
    let result = tms::expand_windows(vec!["my-project".to_string()], &active, &h.tmux);

    // "my-project" normalizes to "my_project" which matches the active session
    // Should be expanded since the session has 2 windows
    assert!(
        result.iter().any(|e| e.starts_with("my-project/")),
        "normalized name should match and expand: {:?}",
        result
    );
}

#[test]
fn test_new_window_detached_does_not_change_active() {
    let h = TmuxHarness::new();
    h.create_session("project", &[]);

    // Get current active window
    let before = h.run_tmux(&[
        "display-message",
        "-t",
        "project",
        "-p",
        "#{window_index}",
    ]);

    // Create a detached window
    h.tmux
        .new_window_detached(Some("background"), None, Some("project"), true);

    // Active window should not have changed
    let after = h.run_tmux(&[
        "display-message",
        "-t",
        "project",
        "-p",
        "#{window_index}",
    ]);

    assert_eq!(
        before.trim(),
        after.trim(),
        "detached window creation should not change active window"
    );
}

#[test]
fn test_kill_window() {
    let h = TmuxHarness::new();
    h.create_session("cleanup", &["to-remove", "to-keep"]);

    let before = h.tmux.list_session_windows("cleanup");
    let before_count = before.len();

    h.tmux.kill_window("cleanup:to-remove");

    let after = h.tmux.list_session_windows("cleanup");
    assert_eq!(
        after.len(),
        before_count - 1,
        "should have one fewer window after kill"
    );
}
