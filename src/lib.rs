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
