# Auto-Refresh Worktree Windows Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Automatically sync tmux windows with git worktrees via a global `pane-focus-in` hook, debounced with a timestamp file.

**Architecture:** Add a `set_hook` method to `Tmux`, install the hook during session creation, and add debounce logic to the refresh command. The hook calls `tms refresh` which already handles idempotent window creation.

**Tech Stack:** Rust, tmux `set-hook` command, `std::time::SystemTime` for timestamps.

---

### Task 1: Add `set_hook` method to `Tmux`

**Files:**
- Modify: `src/tmux.rs:212` (in the "miscellaneous" section)

**Step 1: Write the method**

Add after the `// miscellaneous` comment block (after `select_window`, before `send_keys`):

```rust
pub fn set_hook(&self, hook_name: &str, command: &str) -> process::Output {
    self.execute_tmux_command(&["set-hook", "-g", hook_name, &format!("run-shell \"{command}\"")])
}
```

**Step 2: Verify it compiles**

Run: `cargo check`
Expected: compiles with no errors

**Step 3: Commit**

```bash
git add src/tmux.rs
git commit -m "feat: add set_hook method to Tmux"
```

---

### Task 2: Add debounce logic to `refresh_command`

**Files:**
- Modify: `src/cli.rs:619` (`refresh_command` function)

**Step 1: Add debounce check at the start of `refresh_command`**

At the top of `refresh_command` (line 619), before any existing logic, add an early return if debounce period hasn't elapsed:

```rust
fn refresh_command(args: &RefreshCommand, config: Config, tmux: &Tmux) -> Result<()> {
    // Debounce: skip if last refresh was less than 5 seconds ago
    let debounce_file = dirs::config_dir()
        .or_else(dirs::home_dir)
        .map(|d| d.join("tms/.last-refresh"))
        .ok_or(TmsError::ConfigError)
        .attach_printable("Could not determine config directory")?;

    if let Ok(metadata) = std::fs::metadata(&debounce_file) {
        if let Ok(modified) = metadata.modified() {
            if modified.elapsed().unwrap_or_default() < std::time::Duration::from_secs(5) {
                return Ok(());
            }
        }
    }

    // ... existing refresh logic unchanged ...

    // Write debounce timestamp after successful refresh
    if let Some(parent) = debounce_file.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::File::create(&debounce_file);

    Ok(())
}
```

Key points:
- Uses `dirs::config_dir()` (same pattern as config loading in `configs.rs:173`)
- Falls back to `dirs::home_dir()` (same fallback pattern)
- File path: `<config_dir>/tms/.last-refresh` — inside the tms config directory
- Debounce failures are silent (uses `let _ =`) — refresh should never fail due to debounce issues
- `std::fs::File::create` updates mtime, so we just check `metadata.modified()`

**Step 2: Verify it compiles**

Run: `cargo check`
Expected: compiles with no errors

**Step 3: Test manually**

Run: `cargo run -- refresh` twice in quick succession.
Expected: First call refreshes normally, second call returns immediately (no new windows created).

**Step 4: Commit**

```bash
git add src/cli.rs
git commit -m "feat: add 5-second debounce to refresh command"
```

---

### Task 3: Install global hook on session creation

**Files:**
- Modify: `src/tmux.rs:252` (`set_up_tmux_env` method)
- Modify: `src/session.rs:75` (`switch_to_bookmark_session` method)

The hook needs to be installed in two places where sessions are created:
1. `set_up_tmux_env` — called for git repo sessions (covers `switch_to_repo_session`, `clone_repo_command`, `init_repo_command`)
2. `switch_to_bookmark_session` — called for bookmark sessions

**Step 1: Add `install_refresh_hook` method to `Tmux`**

Add to `src/tmux.rs` after `set_hook`:

```rust
pub fn install_refresh_hook(&self) {
    self.set_hook("pane-focus-in", "tms refresh");
}
```

**Step 2: Call it in `set_up_tmux_env`**

At the start of `set_up_tmux_env` in `src/tmux.rs:252`, add:

```rust
pub fn set_up_tmux_env(
    &self,
    repo: &RepoProvider,
    repo_name: &str,
    config: &Config,
) -> Result<()> {
    self.install_refresh_hook();
    // ... rest unchanged
```

**Step 3: Call it in `switch_to_bookmark_session`**

In `src/session.rs:75`, add the hook installation when creating a new bookmark session:

```rust
fn switch_to_bookmark_session(&self, tmux: &Tmux, path: &Path, config: &Config) -> Result<()> {
    let session_name = self.name.replace('.', "_");

    if !tmux.session_exists(&session_name) {
        tmux.new_session(Some(&session_name), path.to_str());
        tmux.install_refresh_hook();
        tmux.run_session_create_script(path, &session_name, config)?;
    }

    tmux.switch_to_session(&session_name);

    Ok(())
}
```

**Step 4: Verify it compiles**

Run: `cargo check`
Expected: compiles with no errors

**Step 5: Run clippy**

Run: `cargo clippy --all-targets --all-features`
Expected: no warnings

**Step 6: Run tests**

Run: `cargo test`
Expected: all existing tests pass

**Step 7: Commit**

```bash
git add src/tmux.rs src/session.rs
git commit -m "feat: install global pane-focus-in hook on session creation"
```

---

### Task 4: Manual integration test

**Step 1: Build and test the full flow**

```bash
cargo build
```

**Step 2: Test the hook is installed**

1. Open a tmux session via tms (select any repo)
2. Run: `tmux show-hooks -g | grep pane-focus-in`
3. Expected: `pane-focus-in -> run-shell "tms refresh"`

**Step 3: Test auto-refresh**

1. In a git repo session, run `git worktree add ../test-worktree -b test-branch`
2. Switch to another pane or window and back
3. Expected: a new tmux window named `test-branch` appears within 5 seconds

**Step 4: Test debounce**

1. Rapidly switch between panes multiple times
2. Expected: no visible lag or duplicate windows

**Step 5: Clean up test worktree**

```bash
git worktree remove ../test-worktree
```
