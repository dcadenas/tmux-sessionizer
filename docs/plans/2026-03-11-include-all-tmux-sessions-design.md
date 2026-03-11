# Include all tmux sessions in tms pickers

## Problem

The main `tms` picker only shows git repos found in `search_dirs`. The `tms switch` picker can filter out "unknown" sessions via `switch_filter_unknown`. Sessions created manually or by other tools are invisible.

## Solution

Merge active tmux sessions into both pickers. Non-repo sessions appear alongside repo sessions with no visual distinction. Selecting a non-repo session just switches to it.

## Changes

### Main picker (`main.rs::get_session_list`)

After building the repo list, query `tmux list-sessions` for all session names. Append any not already present. They participate in LastAttached sorting using their tmux timestamps.

In `main.rs`, after selection: if `sessions.find_session()` returns `None`, fall back to `tmux.switch_client()` directly.

### Switch picker (`cli.rs::switch_command`)

Remove the `switch_filter_unknown` conditional block. Always show all tmux sessions.

### What doesn't change

- Session creation logic (repos get full setup, worktrees, hooks)
- Preview panes (non-repo sessions won't have directory preview)
- The Session model (no new variants)
