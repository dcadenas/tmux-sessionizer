# Window-aware fuzzy finding

## Problem

The picker only matches on session names. You can't find a session by the name of a window inside it (e.g. a worktree branch name).

## Solution

Expand active multi-window sessions into flat `session/window-name` entries in both pickers. Single-window sessions stay as plain `session`. Selection parses the `/` to switch session + focus window.

## Display format

- Multi-window active session: `ouija/master`, `ouija/test-branch` (one entry per window)
- Single-window active session: `ouija` (no expansion)
- Inactive repo (not yet open): `ouija` (plain name, existing behavior)

## Selection behavior

- Entry with `/`: parse `session/window`, call `switch_client(session)` then `select_window(window)`
- Entry without `/`: existing logic (find_session + switch_to, or fallback switch_client)

## Where it applies

- Main `tms` picker: active sessions expanded, inactive repos stay as plain names
- `tms switch`: all sessions expanded

## Preview

For `session/window` entries, capture pane from the specific window instead of the session's default pane.
