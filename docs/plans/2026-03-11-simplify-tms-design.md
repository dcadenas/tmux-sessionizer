# Simplify tms for personal use

## Goal

Strip unused features from the forked tmux-sessionizer to reduce complexity and prepare for future Claude Code / ouija integration.

## Remove

| Feature | Files affected | Est. lines |
|---------|---------------|------------|
| Jujutsu VCS | repos.rs, configs.rs, Cargo.toml | ~250 |
| Marks | marks.rs (entire), cli.rs, configs.rs | ~200 |
| Bookmarks | cli.rs, configs.rs, session.rs | ~100 |
| Submodule support | repos.rs, session.rs, configs.rs | ~100 |
| `tms start` | cli.rs | ~110 |
| `switch_filter_unknown` | configs.rs, cli.rs (already no-op) | ~20 |

## Keep

- Core git repo discovery + session creation
- Fuzzy picker with preview, keymaps, colors
- Worktree management + auto-refresh hook
- Clone/Init/Create-hook
- Rename, Kill, Switch, Windows, Sessions, Refresh
- Session create scripts
- Config system (trimmed)
- LastAttached sorting
- All tmux sessions in picker (new feature)

## Approach

Surgical removal per feature. Delete code, config fields, imports. Ensure compilation after each removal. Remove jujutsu dependency from Cargo.toml.
