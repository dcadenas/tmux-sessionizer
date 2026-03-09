# Auto-refresh worktree windows via tmux hook

## Problem

Worktree windows only update on session creation or manual `tms refresh`. Adding/removing git worktrees while working doesn't reflect in tmux until manually refreshed.

## Solution

Set a global tmux `pane-focus-in` hook that runs `tms refresh --bare-only` automatically, with a 5-second debounce to avoid redundant work.

## Mechanism

- On every session switch via tms, set `tmux set-hook -g pane-focus-in 'run-shell "tms refresh --bare-only"'`
- Idempotent: re-setting the same hook just overwrites the previous one
- Runs on every pane focus event across all sessions
- Only creates worktree windows for bare repos (where worktree-per-window is the workflow)
- Manual `tms refresh` still works for all repo types

## Debounce

- Timestamp file at `<tms-config-dir>/.last-refresh`
- Before refreshing, check if < 5 seconds since last refresh; skip if so
- Write timestamp after completing refresh
- Hardcoded 5-second duration
- Clock skew defaults to "proceed with refresh"

## Changes

1. `tmux.rs` — `set_hook`, `install_refresh_hook`, `new_window_detached` methods
2. `cli.rs` — debounce logic in refresh, `--bare-only` hidden flag, detached window creation
3. `session.rs` — hook installation in `switch_to` (runs on every session open)

## Non-changes

- Refresh logic for manual `tms refresh` unchanged (works on all repo types)
- Config struct unchanged (no new options)
- No new user-facing subcommands
