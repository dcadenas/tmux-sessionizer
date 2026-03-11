# Simplify tms — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Remove unused features (jujutsu, marks, bookmarks, submodules, start command, switch_filter_unknown) to simplify the codebase for personal use.

**Architecture:** Surgical deletion per feature group. Each task removes one feature completely, compiles, and commits. Order matters: remove jujutsu first (biggest, touches most files), then smaller features.

**Tech Stack:** Rust, Cargo

---

### Task 1: Remove Jujutsu VCS support

**Files:**
- Modify: `Cargo.toml` — remove `jj-lib` dependency
- Modify: `src/repos.rs` — remove all jj imports, `open_jj()`, all Jujutsu match arms in RepoProvider methods
- Modify: `src/configs.rs` — remove `VcsProviders` enum, `vcs_providers` field from Config/ConfigExport, `DEFAULT_VCS_PROVIDERS`
- Modify: `src/cli.rs` — remove any vcs_providers config args if present

**Step 1: Remove jj-lib from Cargo.toml**

Remove the `jj-lib = "0.38.0"` line.

**Step 2: Remove all jj code from repos.rs**

- Remove jj_lib imports (lines 4-12)
- Remove `open_jj()` function (lines 82-106)
- Remove `VcsProviders` import (line 21)
- In `RepoProvider::open()`, remove the VcsProviders matching loop and just call `open_git()` directly
- Remove all `RepoProvider::Jujutsu(..)` match arms from: `is_worktree`, `path`, `main_repo`, `work_dir`, `head_name`, `submodules`, `is_bare`, `add_worktree`, `worktrees`
- Since only Git remains, consider simplifying RepoProvider from an enum to just the Git variant, or leave as single-variant enum

**Step 3: Remove VcsProviders from configs.rs**

- Remove `VcsProviders` enum (lines 61-67)
- Remove `DEFAULT_VCS_PROVIDERS` constant
- Remove `vcs_providers` field from Config and ConfigExport
- Remove vcs_providers unwrap from `From<Config> for ConfigExport`

**Step 4: Run `cargo clippy --all-targets --all-features` and `cargo test`**

Fix any compilation errors from dangling references.

**Step 5: Commit**

```
refactor: remove jujutsu VCS support
```

---

### Task 2: Remove Marks system

**Files:**
- Delete: `src/marks.rs`
- Modify: `src/lib.rs` — remove `pub mod marks;`
- Modify: `src/cli.rs` — remove marks import, `Marks(MarksCommand)` variant, marks handler
- Modify: `src/configs.rs` — remove `marks` field from Config/ConfigExport, remove marks methods (add_mark, delete_mark, clear_marks)

**Step 1: Delete src/marks.rs**

**Step 2: Remove marks from lib.rs, cli.rs, configs.rs**

- lib.rs: remove `pub mod marks;`
- cli.rs: remove `marks::{marks_command, MarksCommand}` import, `Marks(MarksCommand)` enum variant, marks handler in `handle_sub_commands`
- configs.rs: remove `marks` field from Config (line 54), ConfigExport (line 84), From impl (line 111), and all marks methods (lines 289-309)

**Step 3: Run `cargo clippy --all-targets --all-features` and `cargo test`**

**Step 4: Commit**

```
refactor: remove marks system
```

---

### Task 3: Remove Bookmarks

**Files:**
- Modify: `src/cli.rs` — remove BookmarkCommand struct, Bookmark variant, handler, bookmark_command function
- Modify: `src/configs.rs` — remove bookmarks field, methods
- Modify: `src/session.rs` — remove Bookmark variant from SessionType, append_bookmarks function, switch_to_bookmark_session

**Step 1: Remove from cli.rs**

- Remove `BookmarkCommand` struct (lines 174-180)
- Remove `Bookmark(BookmarkCommand)` variant from CliCommand
- Remove bookmark handler in handle_sub_commands
- Remove `bookmark_command()` function (lines 816-833)

**Step 2: Remove from configs.rs**

- Remove `bookmarks` field from Config/ConfigExport
- Remove bookmarks unwrap from From impl
- Remove bookmark methods (add_bookmark, delete_bookmark, bookmark_paths — lines 250-287)

**Step 3: Remove from session.rs**

- Remove `Bookmark(PathBuf)` variant from SessionType
- Remove `append_bookmarks()` call in `create_sessions()`
- Remove `switch_to_bookmark_session()` method
- Remove `append_bookmarks()` function

**Step 4: Run `cargo clippy --all-targets --all-features` and `cargo test`**

**Step 5: Commit**

```
refactor: remove bookmarks
```

---

### Task 4: Remove Submodule support

**Files:**
- Modify: `src/repos.rs` — remove `find_submodules()` function, `submodules()` method
- Modify: `src/session.rs` — remove find_submodules import and call
- Modify: `src/cli.rs` — remove search_submodules/recursive_submodules config args
- Modify: `src/configs.rs` — remove search_submodules/recursive_submodules fields

**Step 1: Remove from repos.rs**

- Remove `find_submodules()` function (lines 364-401)
- Remove `submodules()` method from RepoProvider (lines 170-175)

**Step 2: Remove from session.rs**

- Remove `find_submodules` import
- Remove submodule handling in `insert_session()` or wherever it's called

**Step 3: Remove from cli.rs and configs.rs**

- cli.rs: remove search_submodules/recursive_submodules args and config setters
- configs.rs: remove both fields from Config/ConfigExport/From impl

**Step 4: Run `cargo clippy --all-targets --all-features` and `cargo test`**

**Step 5: Commit**

```
refactor: remove submodule support
```

---

### Task 5: Remove `tms start` and `switch_filter_unknown`

**Files:**
- Modify: `src/cli.rs` — remove Start variant, handler, start_command function; remove switch_filter_unknown arg and config setter
- Modify: `src/configs.rs` — remove switch_filter_unknown field

**Step 1: Remove from cli.rs**

- Remove `Start` variant from CliCommand enum
- Remove Start handler in handle_sub_commands
- Remove `start_command()` function (lines 269-305)
- Remove `switch_filter_unknown` config arg and setter

**Step 2: Remove from configs.rs**

- Remove `switch_filter_unknown` from Config/ConfigExport/From impl

**Step 3: Run `cargo clippy --all-targets --all-features` and `cargo test`**

**Step 4: Commit**

```
refactor: remove start command and switch_filter_unknown
```

---

### Task 6: Final cleanup and verify

**Step 1: Run `cargo clippy --all-targets --all-features`**

Fix any remaining warnings from unused imports, dead code, etc.

**Step 2: Run `cargo test`**

Verify the config test in tests/cli.rs still passes. It references `switch_filter_unknown` — update or remove that assertion.

**Step 3: Build and install**

```bash
cargo install --path .
```

**Step 4: Manual verification**

- `tms` — picker works, shows repos + active sessions
- `tms switch` — shows all sessions
- `tms refresh` — worktree refresh works

**Step 5: Commit any fixups**

```
chore: final cleanup after simplification
```
