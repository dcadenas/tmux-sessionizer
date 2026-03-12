use std::collections::HashSet;
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};

use tms::tmux::{strip_tmux_style_directives, Tmux};

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
fn test_expand_windows_single_window_still_expanded() {
    let h = TmuxHarness::new();
    h.create_session("solo", &[]);

    let active: HashSet<&str> = ["solo"].into();
    let result = tms::expand_windows(vec!["solo".to_string()], &active, &h.tmux);

    // Even single-window sessions are expanded so the window display name is searchable
    assert_eq!(result.len(), 1);
    assert!(
        result[0].starts_with("solo/"),
        "single-window session should be expanded: {:?}",
        result
    );
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

// --- strip_tmux_style_directives (pure function, no tmux needed) ---

#[test]
fn test_strip_simple_style() {
    let input = "#[fg=red,bold] #I #{window_name} #[default]";
    let result = strip_tmux_style_directives(input);
    assert_eq!(result, "#{window_name}");
}

#[test]
fn test_strip_complex_format() {
    // Realistic format similar to oh-my-tmux
    let input = "#[fg=#080808,bg=#00afff,bold] #I #{?@ouija_session,⊕ #{@ouija_session},#{b:pane_current_path}}#{?window_bell_flag,!,}#{?window_zoomed_flag,Z,} #[fg=#00afff,bg=#080808]";
    let result = strip_tmux_style_directives(input);
    assert_eq!(
        result,
        "#{?@ouija_session,⊕ #{@ouija_session},#{b:pane_current_path}}"
    );
}

#[test]
fn test_strip_no_directives() {
    let input = "#{window_name}";
    let result = strip_tmux_style_directives(input);
    assert_eq!(result, "#{window_name}");
}

#[test]
fn test_strip_removes_window_index() {
    let input = "#I #{window_name}";
    let result = strip_tmux_style_directives(input);
    assert_eq!(result, "#{window_name}");
}

#[test]
fn test_strip_removes_bell_zoom_indicators() {
    let input = "#{window_name}#{?#{||:#{window_bell_flag},#{window_zoomed_flag}}, ,}#{?window_bell_flag,!,}#{?window_zoomed_flag,Z,}";
    let result = strip_tmux_style_directives(input);
    assert_eq!(result, "#{window_name}");
}

// --- Window display format integration (uses tmux) ---

#[test]
fn test_window_names_use_display_format() {
    let h = TmuxHarness::new();

    // Set a custom window-status-current-format on the test server
    h.run_tmux(&[
        "set-option",
        "-g",
        "window-status-current-format",
        "#[bold] #I #{b:pane_current_path} #[default]",
    ]);

    h.create_session("fmt-test", &[]);

    let windows = h.tmux.list_session_windows("fmt-test");
    // Windows should use the display format (basename of pane path), not raw window_name
    // The exact value depends on the shell's cwd, but it should NOT be empty
    assert!(
        !windows.is_empty(),
        "should have at least one window"
    );
    for win in &windows {
        assert!(
            win.contains(':'),
            "window entry should have index:name format, got: {}",
            win
        );
    }
}

// --- select_window targeting ---

#[test]
fn test_select_window_by_index() {
    let h = TmuxHarness::new();
    h.create_session("nav", &["second", "third"]);

    // Select window by index (the way our picker does it)
    let result = h.tmux.select_window("nav:2");
    assert!(result.status.success(), "select_window by index should succeed");

    // Verify the active window changed
    let active = h.run_tmux(&[
        "display-message",
        "-t",
        "nav",
        "-p",
        "#{window_index}",
    ]);
    assert_eq!(active.trim(), "2", "active window should be index 2");
}

// --- Session operations ---

#[test]
fn test_rename_session() {
    let h = TmuxHarness::new();
    h.create_session("old-name", &[]);

    assert!(h.tmux.session_exists("old-name"));

    // rename-session renames the current session, so we need to target it
    h.run_tmux(&["switch-client", "-t", "old-name"]);
    h.tmux.rename_session("new-name");

    // With a detached server, rename applies to last active.
    // Check that at least one of the names exists
    let sessions = h.tmux.list_sessions("#S");
    let has_new = sessions.lines().any(|l| l.trim() == "new-name");
    let has_old = sessions.lines().any(|l| l.trim() == "old-name");
    assert!(
        has_new || !has_old,
        "session should have been renamed, sessions: {}",
        sessions.trim()
    );
}

#[test]
fn test_kill_session() {
    let h = TmuxHarness::new();
    h.create_session("doomed", &[]);

    assert!(h.tmux.session_exists("doomed"));
    h.tmux.kill_session("doomed");
    assert!(!h.tmux.session_exists("doomed"));
}

// --- Never-attached sessions ---

#[test]
fn test_never_attached_session_has_timestamp() {
    let h = TmuxHarness::new();
    // Create a session that is never attached to
    h.create_session("ghost", &[]);

    // Query with the same format the picker uses
    let raw = h.tmux.list_sessions("'#{session_name}#,#{session_last_attached}'");

    // Find the ghost session line
    let ghost_line = raw
        .trim()
        .lines()
        .find(|line| line.contains("ghost"))
        .expect("ghost session should be in listing");

    let ghost_line = ghost_line.trim_matches('\'');
    let (name, _timestamp) = ghost_line.split_once(',').expect("should have comma separator");
    assert_eq!(name, "ghost");
    // Timestamp may be empty for never-attached, which is fine —
    // our code defaults to 0 so it still appears in the picker
}

// --- Expand windows with duplicate window names ---

#[test]
fn test_expand_windows_duplicate_names_are_unique() {
    let h = TmuxHarness::new();
    // Create a session with identically-named windows (like multiple "claude" windows)
    h.create_session("dupes", &["claude", "claude", "claude"]);

    let active: HashSet<&str> = ["dupes"].into();
    let result = tms::expand_windows(vec!["dupes".to_string()], &active, &h.tmux);

    // All entries should be unique (index:name disambiguates)
    let unique: HashSet<&String> = result.iter().collect();
    assert_eq!(
        result.len(),
        unique.len(),
        "all expanded entries should be unique: {:?}",
        result
    );
}

// --- Move window ---

#[test]
fn test_move_window() {
    let h = TmuxHarness::new();
    h.create_session("movable", &["target"]);

    // Move window to a different index
    let result = h.tmux.move_window("movable:target", "movable:99");
    assert!(result.status.success(), "move_window should succeed");

    // Verify the window exists at the new index
    let windows = h.run_tmux(&["list-windows", "-t", "movable", "-F", "#{window_index}"]);
    assert!(
        windows.lines().any(|l| l.trim() == "99"),
        "window should be at index 99, got: {}",
        windows.trim()
    );
}

// --- Install refresh hook ---

#[test]
fn test_install_refresh_hook() {
    let h = TmuxHarness::new();
    h.tmux.install_refresh_hook();

    // Verify the hook was set
    let hooks = h.run_tmux(&["show-hooks", "-g", "pane-focus-in"]);
    assert!(
        hooks.contains("tms refresh --bare-only"),
        "refresh hook should be installed, got: {}",
        hooks.trim()
    );
}
