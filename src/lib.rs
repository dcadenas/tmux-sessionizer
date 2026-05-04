pub mod cli;
pub mod configs;
pub mod dirty_paths;
pub mod error;
pub mod keymap;
pub mod picker;
pub mod repos;
pub mod session;
pub mod tmux;

use configs::Config;
use std::{collections::HashSet, process};

use crate::{
    error::{Result, TmsError},
    picker::{Picker, Preview},
    tmux::Tmux,
};

pub fn execute_command(command: &str, args: Vec<String>) -> process::Output {
    process::Command::new(command)
        .args(args)
        .stdin(process::Stdio::inherit())
        .output()
        .unwrap_or_else(|_| panic!("Failed to execute command `{command}`"))
}

/// Parse a picker entry of form `<session_name>/<window_index>:<window_display>`.
///
/// Returns `Some((session_name, window_part))` where `window_part` includes the
/// index and display (e.g., `"1:branch"`). Returns `None` if the entry is not
/// an expanded session/window entry (a plain session name or a path-based
/// deduplicated name).
///
/// Both the session name and the window display can contain `/`, so we locate
/// the boundary by looking for a `/` followed by `<digits>:` — the window
/// index pattern emitted by [`Tmux::list_session_windows`].
pub fn parse_session_window_entry(entry: &str) -> Option<(&str, &str)> {
    let mut search = 0;
    while let Some(slash_offset) = entry[search..].find('/') {
        let slash_pos = search + slash_offset;
        let after = &entry[slash_pos + 1..];
        let digit_count = after.bytes().take_while(|b| b.is_ascii_digit()).count();
        if digit_count > 0 && after.as_bytes().get(digit_count) == Some(&b':') {
            return Some((&entry[..slash_pos], after));
        }
        search = slash_pos + 1;
    }
    None
}

/// Expand active sessions into `session/window` entries.
pub fn expand_windows(
    sessions: Vec<String>,
    active_names: &HashSet<&str>,
    tmux: &Tmux,
) -> Vec<String> {
    let mut result = Vec::new();
    for name in sessions {
        let normalized = name.replace(['.', '-'], "_");
        let is_active = active_names.contains(name.as_str())
            || active_names.contains(normalized.as_str());
        if is_active {
            // Try the original name first (tmux keeps hyphens), fall back to normalized
            let windows = tmux.list_session_windows(&name);
            let windows = if windows.is_empty() {
                tmux.list_session_windows(&normalized)
            } else {
                windows
            };
            if windows.is_empty() {
                result.push(name);
            } else {
                for win in &windows {
                    result.push(format!("{}/{}", name, win));
                }
            }
        } else {
            result.push(name);
        }
    }
    result
}

pub fn get_single_selection(
    list: &[String],
    preview: Option<Preview>,
    config: &Config,
    tmux: &Tmux,
) -> Result<Option<String>> {
    let mut picker = Picker::new(
        list,
        preview,
        config.shortcuts.as_ref(),
        config.input_position.unwrap_or_default(),
        tmux,
    )
    .set_colors(config.picker_colors.as_ref());

    picker.run()
}

#[cfg(test)]
mod tests {
    use super::parse_session_window_entry;

    #[test]
    fn parses_simple_session_window() {
        assert_eq!(
            parse_session_window_entry("session/1:window"),
            Some(("session", "1:window"))
        );
    }

    #[test]
    fn parses_window_display_with_slash() {
        // The display string may contain '/' (e.g., from @ouija_session showing
        // a branch like "feat/foo"). The first '/' followed by '<digits>:' is the boundary.
        assert_eq!(
            parse_session_window_entry("divine-funnelcake/1:⊕ feat/323-add-endpoint"),
            Some(("divine-funnelcake", "1:⊕ feat/323-add-endpoint"))
        );
    }

    #[test]
    fn parses_session_name_with_slash() {
        // Path-based deduplicated session name like "parent/projectname".
        assert_eq!(
            parse_session_window_entry("parent/projectname/1:branch"),
            Some(("parent/projectname", "1:branch"))
        );
    }

    #[test]
    fn parses_multi_digit_index() {
        assert_eq!(
            parse_session_window_entry("session/12:window"),
            Some(("session", "12:window"))
        );
    }

    #[test]
    fn returns_none_for_plain_session_name() {
        assert_eq!(parse_session_window_entry("session"), None);
    }

    #[test]
    fn returns_none_for_dedup_session_without_window() {
        // Path-based name that is not an active expanded entry.
        assert_eq!(parse_session_window_entry("parent/projectname"), None);
    }

    #[test]
    fn parses_session_with_dots() {
        assert_eq!(
            parse_session_window_entry("session.with.dots/1:window"),
            Some(("session.with.dots", "1:window"))
        );
    }
}
