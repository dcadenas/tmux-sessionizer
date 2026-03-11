# Include All Tmux Sessions in Pickers — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make all active tmux sessions visible in both the main `tms` picker and `tms switch`, not just git-repo-backed sessions.

**Architecture:** Merge tmux's live session list into the picker's item list at the presentation layer. Non-repo sessions get a simple `switch_client` fallback. No changes to the Session model.

**Tech Stack:** Rust, tmux CLI queries via `Tmux` struct

---

### Task 1: Add tmux session merging to `get_session_list`

**Files:**
- Modify: `src/main.rs:84-151` (`get_session_list` function)

**Step 1: Modify the LastAttached branch**

In the `if matches!(... LastAttached)` branch, after partitioning repo sessions into active/inactive (line 127), append any tmux session names that aren't in the repo list. The `active_sessions` vec already has all tmux sessions with timestamps — iterate it and add any name not found in `active_list` or `inactive_list` to `active_list` (since they're active in tmux).

```rust
// After line 145: active_list.extend(inactive_list);
// Add tmux-only sessions (active in tmux but not in repo list)
let all_names: HashSet<&str> = active_list.iter().map(|s| s.as_str()).collect();
for (name, _) in &active_sessions {
    let normalized = name.replace('_', "-");
    if !all_names.contains(name) && !all_names.contains(normalized.as_str()) {
        active_list.push(name.to_string());
    }
}
```

Note: tmux normalizes dots/hyphens to underscores in session names, so the reverse lookup needs to account for that.

**Step 2: Modify the default (alphabetical) branch**

In the `else` branch (line 147-150), also query tmux and append unknown sessions.

```rust
} else {
    let mut all = all_sessions;
    let tmux_sessions_raw = tmux.list_sessions("#S");
    let existing: HashSet<String> = all.iter().cloned().collect();
    for name in tmux_sessions_raw.trim().split('\n') {
        let name = name.trim();
        if !name.is_empty() && !existing.contains(name) {
            all.push(name.to_string());
        }
    }
    all.sort();
    (all, None)
}
```

**Step 3: Run `cargo clippy --all-targets --all-features`**

Expected: no new warnings

**Step 4: Commit**

```
feat: include all tmux sessions in main picker list
```

---

### Task 2: Add fallback switch for non-repo sessions in main picker

**Files:**
- Modify: `src/main.rs:77-79` (selection handler)

**Step 1: Add fallback after `find_session`**

Replace the current selection handler:

```rust
// Before:
if let Some(session) = sessions.find_session(&selected_str) {
    session.switch_to(&tmux, &config)?;
}

// After:
if let Some(session) = sessions.find_session(&selected_str) {
    session.switch_to(&tmux, &config)?;
} else {
    // Non-repo tmux session — just switch to it
    tmux.switch_client(&selected_str.replace('.', "_"));
}
```

**Step 2: Run `cargo clippy --all-targets --all-features`**

Expected: no new warnings

**Step 3: Commit**

```
feat: switch to non-repo tmux sessions from main picker
```

---

### Task 3: Remove `switch_filter_unknown` from `tms switch`

**Files:**
- Modify: `src/cli.rs:323-331` (`switch_command`)

**Step 1: Remove the filter block**

Delete lines 324-331:

```rust
// Remove this block:
if let Some(true) = config.switch_filter_unknown {
    let configured = create_sessions(&config)?;

    sessions = sessions
        .into_iter()
        .filter(|session| configured.find_session(session).is_some())
        .collect::<Vec<String>>();
}
```

**Step 2: Run `cargo clippy --all-targets --all-features`**

Expected: no new warnings. Check if `switch_filter_unknown` or `create_sessions` import becomes unused.

**Step 3: Run `cargo test`**

Expected: all tests pass. The config test in `tests/cli.rs` sets `switch_filter_unknown: Some(false)` — the field still exists in Config, it's just ignored now.

**Step 4: Commit**

```
feat: always show all sessions in tms switch
```

---

### Task 4: Build, install, and verify

**Step 1: Build release binary**

```bash
cargo install --path .
```

**Step 2: Manual verification**

- Run `tms` — verify non-repo tmux sessions appear in picker
- Select a non-repo session — verify it switches correctly
- Run `tms switch` — verify all sessions appear (no filtering)
- Select a session in `tms switch` — verify it switches

**Step 3: Commit any fixups if needed**
