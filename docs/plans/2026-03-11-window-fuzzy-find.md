# Window-Aware Fuzzy Finding — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Expand active multi-window sessions into `session/window-name` entries in both pickers so users can fuzzy-find by window name and jump directly to that window.

**Architecture:** Build window-expanded session lists in `main.rs::get_session_list()` and `cli.rs::switch_command()` by querying tmux for windows per active session. On selection, parse the `/` delimiter to determine session + window, then switch_client + select_window. Update preview to target specific windows.

**Tech Stack:** Rust, tmux CLI, Nucleo fuzzy matcher

---

### Task 1: Add `list_session_windows` helper to Tmux

**Files:**
- Modify: `src/tmux.rs`

**Step 1: Add method to Tmux struct**

Add a method that returns all windows for a given session as `Vec<String>`:

```rust
pub fn list_session_windows(&self, session: &str) -> Vec<String> {
    let output = self.list_windows("'#{window_name}'", Some(session));
    output
        .trim()
        .split('\n')
        .map(|line| line.replace('\'', "").trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}
```

**Step 2: Run `cargo check`**

**Step 3: Commit**

```
feat: add list_session_windows helper to Tmux
```

---

### Task 2: Expand active sessions into session/window entries in main picker

**Files:**
- Modify: `src/main.rs:87-180` (`get_session_list` function)

**Step 1: Add helper function to expand session names**

Add a function that takes a list of session names and a `Tmux` reference, and returns expanded entries. For each active tmux session with >1 window, replace the single entry with `session/window` entries for each window. Single-window sessions stay as-is.

```rust
fn expand_windows(sessions: Vec<String>, active_names: &HashSet<&str>, tmux: &Tmux) -> Vec<String> {
    let mut result = Vec::new();
    for name in sessions {
        let normalized = name.replace(['.', '-'], "_");
        let is_active = active_names.contains(name.as_str())
            || active_names.contains(normalized.as_str());
        if is_active {
            let tmux_name = normalized;
            let windows = tmux.list_session_windows(&tmux_name);
            if windows.len() > 1 {
                for win in &windows {
                    result.push(format!("{}/{}", name, win));
                }
            } else {
                result.push(name);
            }
        } else {
            result.push(name);
        }
    }
    result
}
```

**Step 2: Call expand_windows in the LastAttached branch**

After `active_list.extend(inactive_list)` and the tmux session merging block, call:

```rust
let active_names_ref: HashSet<&str> = active_names.iter().map(|s| s.as_str()).collect();
let active_list = expand_windows(active_list, &active_names_ref, tmux);
```

Return `(active_list, Some(active_names_owned))`.

**Step 3: Call expand_windows in the alphabetical branch**

After merging tmux sessions and sorting, expand:

```rust
let tmux_sessions_raw_for_active = tmux.list_sessions("#S");
let active_set: HashSet<&str> = tmux_sessions_raw_for_active.trim().split('\n')
    .map(|s| s.trim())
    .filter(|s| !s.is_empty())
    .collect();
let all = expand_windows(all, &active_set, tmux);
```

**Step 4: Update `active_sessions` set for bold styling**

The active_sessions HashSet used for bold styling needs to include the expanded `session/window` names too. After expanding, rebuild:

```rust
let active_names_owned: HashSet<String> = active_list.iter()
    .filter(|name| {
        let base = name.split('/').next().unwrap_or(name);
        let normalized = base.replace(['.', '-'], "_");
        active_names.contains(base) || active_names.contains(normalized.as_str())
    })
    .cloned()
    .collect();
```

**Step 5: Run `cargo check`**

**Step 6: Commit**

```
feat: expand active sessions into session/window entries in main picker
```

---

### Task 3: Handle session/window selection in main picker

**Files:**
- Modify: `src/main.rs:77-82` (selection handler after picker)

**Step 1: Parse session/window on selection**

Replace the current selection handler:

```rust
if let Some(name) = selected_str.strip_prefix("__TMS_CREATE_NEW__:") {
    create_new_directory(name, &config, &tmux)?;
    return Ok(());
}

if let Some((session_part, window_part)) = selected_str.split_once('/') {
    // session/window entry — switch to session and focus window
    let tmux_session = session_part.replace('.', "_");
    tmux.switch_client(&tmux_session);
    tmux.select_window(&format!("{}:{}", tmux_session, window_part));
} else if let Some(session) = sessions.find_session(&selected_str) {
    session.switch_to(&tmux, &config)?;
} else {
    tmux.switch_client(&selected_str.replace('.', "_"));
}
```

Note: `select_window` already exists in `tmux.rs` — it calls `tmux select-window -t <window>`. The format `session:window_name` tells tmux which session's window to select.

**Step 2: Run `cargo check`**

**Step 3: Commit**

```
feat: handle session/window selection in main picker
```

---

### Task 4: Expand sessions in tms switch

**Files:**
- Modify: `src/cli.rs:307-332` (`switch_command`)

**Step 1: Expand windows in switch_command**

After building the sessions list and sorting, expand multi-window sessions:

```rust
let sessions: Vec<String> = sessions.into_iter().map(|s| s.0.to_string()).collect();

// Expand multi-window sessions into session/window entries
let active_names: HashSet<&str> = sessions.iter().map(|s| s.as_str()).collect();
let sessions = expand_windows(sessions, &active_names, tmux);
```

Import `expand_windows` — but it's in `main.rs`. Either move it to a shared location (e.g. `tmux.rs` or a new helper) or duplicate the logic inline.

Better approach: move `expand_windows` to `src/tmux.rs` as a method on `Tmux`, or to a standalone function in `src/lib.rs`.

**Step 2: Handle session/window selection in switch_command**

Update the selection handler:

```rust
if let Some(target_session) =
    get_single_selection(&sessions, Some(Preview::SessionPane), &config, tmux)?
{
    if let Some((session_part, window_part)) = target_session.split_once('/') {
        let tmux_session = session_part.replace('.', "_");
        tmux.switch_client(&tmux_session);
        tmux.select_window(&format!("{}:{}", tmux_session, window_part));
    } else {
        tmux.switch_client(&target_session.replace('.', "_"));
    }
}
```

**Step 3: Run `cargo clippy --all-targets --all-features`**

**Step 4: Commit**

```
feat: expand sessions and handle window selection in tms switch
```

---

### Task 5: Update preview for session/window entries

**Files:**
- Modify: `src/picker/mod.rs:330-357` (`get_preview_text`)

**Step 1: Update preview to target specific window pane**

In `get_preview_text`, when the selected item contains `/`, capture the pane from that specific window:

```rust
Some(Preview::SessionPane) => {
    if let Some((session, window)) = item_data.split_once('/') {
        let target = format!("{}:{}", session.replace('.', "_"), window);
        self.tmux.capture_pane(&target)
    } else {
        self.tmux.capture_pane(item_data)
    }
}
```

**Step 2: Update active session bold styling**

In the render method, the active session check needs to handle `session/window` entries — extract the session part before checking:

```rust
let base_name = text.split('/').next().unwrap_or(text);
let normalized = base_name.replace(['.', '-'], "_");
if active.contains(base_name) || active.contains(&normalized) {
    return ListItem::new(Span::styled(text, Style::default().bold()));
}
```

**Step 3: Run `cargo clippy --all-targets --all-features`**

**Step 4: Commit**

```
feat: update preview and styling for session/window entries
```

---

### Task 6: Build, install, verify

**Step 1: Run `cargo test`**

**Step 2: Build and install**

```bash
cargo install --path .
```

**Step 3: Manual verification**

- `tms` — sessions with multiple windows show expanded `session/window` entries
- Select a `session/window` entry — switches to session AND focuses that window
- `tms switch` — same expansion and behavior
- Preview pane shows content from the correct window
- Single-window sessions still show as plain names
- Typing a window name filters correctly

**Step 4: Commit any fixups**
