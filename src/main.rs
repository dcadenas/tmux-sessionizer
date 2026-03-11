use std::{collections::HashSet, env, path::PathBuf, process::Command};

use clap::{CommandFactory, Parser};
use clap_complete::CompleteEnv;
use error_stack::{Report, ResultExt};

use tms::{
    cli::{Cli, SubCommandGiven},
    configs::SessionSortOrderConfig,
    error::{Result, Suggestion, TmsError},
    expand_windows,
    session::{create_sessions, SessionContainer},
    tmux::Tmux,
};

fn main() -> Result<()> {
    // Install debug hooks for formatting of error handling
    Report::install_debug_hook::<Suggestion>(|value, context| {
        context.push_body(format!("{value}"));
    });
    #[cfg(any(not(debug_assertions), test))]
    Report::install_debug_hook::<std::panic::Location>(|_value, _context| {});

    let bin_name = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.file_name().map(|exe| exe.to_string_lossy().to_string()))
        .unwrap_or("tms".into());
    match CompleteEnv::with_factory(Cli::command)
        .bin(bin_name)
        .try_complete(env::args_os(), None)
    {
        Ok(true) => return Ok(()),
        Err(e) => {
            panic!("failed to generate completions: {e}");
        }
        Ok(false) => {}
    };

    // Use CLAP to parse the command line arguments
    let cli_args = Cli::parse();

    let tmux = Tmux::default();

    let config = match cli_args.handle_sub_commands(&tmux)? {
        SubCommandGiven::Yes => return Ok(()),
        SubCommandGiven::No(config) => config, // continue
    };

    let sessions = create_sessions(&config)?;
    let (session_strings, active_sessions) = get_session_list(&sessions, &config, &tmux);

    // Create picker with active session styling
    let mut picker = tms::picker::Picker::new(
        &session_strings,
        None,
        config.shortcuts.as_ref(),
        config.input_position.unwrap_or_default(),
        &tmux,
    )
    .set_colors(config.picker_colors.as_ref());

    if let Some(active) = active_sessions {
        picker = picker.set_active_sessions(active);
    }

    let selected_str = if let Some(str) = picker.run()? {
        str
    } else {
        return Ok(());
    };

    // Check if user wants to create a new directory
    if let Some(name) = selected_str.strip_prefix("__TMS_CREATE_NEW__:") {
        create_new_directory(name, &config, &tmux)?;
        return Ok(());
    }

    if let Some((session_part, window_part)) = selected_str.split_once('/') {
        // session/window entry — switch to session and focus window
        let tmux_session = session_part.replace('.', "_");
        // window_part is "index:name" — extract index for tmux target
        let window_target = window_part
            .split_once(':')
            .map_or(window_part, |(idx, _)| idx);
        tmux.switch_client(&tmux_session);
        tmux.select_window(&format!("{}:{}", tmux_session, window_target));
    } else if let Some(session) = sessions.find_session(&selected_str) {
        session.switch_to(&tmux, &config)?;
    } else {
        tmux.switch_client(&selected_str.replace('.', "_"));
    }

    Ok(())
}

/// Get the session list, optionally sorted with active sessions first
/// Returns (session_list, active_sessions_set)
fn get_session_list(
    sessions: &impl SessionContainer,
    config: &tms::configs::Config,
    tmux: &Tmux,
) -> (Vec<String>, Option<HashSet<String>>) {
    let all_sessions = sessions.list();

    // If LastAttached is configured, prioritize active tmux sessions
    if matches!(
        config.session_sort_order,
        Some(SessionSortOrderConfig::LastAttached)
    ) {
        // Get active sessions from tmux with timestamps, excluding the currently attached one
        let active_sessions_raw =
            tmux.list_sessions("'#{?session_attached,,#{session_name}#,#{session_last_attached}}'");

        // Parse into (name, timestamp) pairs
        let active_sessions: Vec<(&str, i64)> = active_sessions_raw
            .trim()
            .split('\n')
            .filter_map(|line| {
                let line = line.trim_matches('\'');
                let (name, timestamp) = line.split_once(',')?;
                let timestamp = timestamp.parse::<i64>().ok()?;
                Some((name, timestamp))
            })
            .collect();

        // Build a set of active session names for fast lookup
        let active_names: HashSet<&str> = active_sessions.iter().map(|(name, _)| *name).collect();

        // Partition sessions into active and inactive
        let (mut active_list, mut inactive_list): (Vec<String>, Vec<String>) =
            all_sessions.into_iter().partition(|session_name| {
                // Check if this session name (or its normalized form) is active
                // Tmux normalizes both dots and hyphens to underscores in session names
                let normalized = session_name.replace(['.', '-'], "_");
                active_names.contains(session_name.as_str())
                    || active_names.contains(&normalized.as_str())
            });

        // Sort active sessions by timestamp (most recent first)
        active_list.sort_by_cached_key(|name| {
            // Find the timestamp for this session
            // Tmux normalizes both dots and hyphens to underscores
            let normalized = name.replace(['.', '-'], "_");
            active_sessions
                .iter()
                .find(|(active_name, _)| *active_name == name || *active_name == normalized)
                .map(|(_, timestamp)| -timestamp) // Negative for descending order
                .unwrap_or(0)
        });

        // Sort inactive sessions alphabetically
        inactive_list.sort();

        // Combine: active first, then inactive
        active_list.extend(inactive_list);

        // Append any tmux sessions not already represented (e.g. manually created)
        let existing: HashSet<String> = active_list.iter().cloned().collect();
        let existing_normalized: HashSet<String> = existing
            .iter()
            .map(|s| s.replace(['.', '-'], "_"))
            .collect();
        for (name, _) in &active_sessions {
            let name = name.trim();
            if name.is_empty() {
                continue;
            }
            let normalized = name.replace(['.', '-'], "_");
            if !existing.contains(name) && !existing_normalized.contains(&normalized) {
                active_list.push(name.to_string());
            }
        }

        // Expand active sessions with multiple windows into session/window entries
        let active_names_ref: HashSet<&str> = active_names.iter().copied().collect();
        let expanded = expand_windows(active_list, &active_names_ref, tmux);

        // Update active_names_owned to include expanded session/window entries
        let mut active_names_owned: HashSet<String> =
            active_names.iter().map(|s| s.to_string()).collect();
        for entry in &expanded {
            if let Some((session_part, _)) = entry.split_once('/') {
                let normalized = session_part.replace(['.', '-'], "_");
                if active_names.contains(session_part)
                    || active_names.contains(normalized.as_str())
                {
                    active_names_owned.insert(entry.clone());
                }
            }
        }

        (expanded, Some(active_names_owned))
    } else {
        // Default behavior: alphabetically sorted
        let mut all = all_sessions;
        let tmux_sessions_raw = tmux.list_sessions("#S");
        let existing: HashSet<String> = all.iter().cloned().collect();
        let existing_normalized: HashSet<String> =
            all.iter().map(|s| s.replace(['.', '-'], "_")).collect();
        for name in tmux_sessions_raw.trim().split('\n') {
            let name = name.trim();
            if !name.is_empty()
                && !existing.contains(name)
                && !existing_normalized.contains(name)
            {
                all.push(name.to_string());
            }
        }
        all.sort();

        // Build active names set from tmux sessions for window expansion
        let tmux_active: HashSet<&str> = tmux_sessions_raw
            .trim()
            .split('\n')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();
        let expanded = expand_windows(all, &tmux_active, tmux);

        (expanded, None)
    }
}

/// Default create hook template embedded in binary
const DEFAULT_HOOK: &str = include_str!("../create-hook");

/// Ensure the create hook exists at the conventional location
fn ensure_hook_exists(hook_path: &PathBuf) -> Result<()> {
    if hook_path.exists() {
        return Ok(());
    }

    // Create parent directory if needed
    if let Some(parent) = hook_path.parent() {
        std::fs::create_dir_all(parent)
            .change_context(TmsError::IoError)
            .attach_printable(format!("Failed to create directory: {}", parent.display()))?;
    }

    // Write default hook
    std::fs::write(hook_path, DEFAULT_HOOK)
        .change_context(TmsError::IoError)
        .attach_printable(format!("Failed to write hook: {}", hook_path.display()))?;

    // Make executable on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(hook_path)
            .change_context(TmsError::IoError)?
            .permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(hook_path, perms)
            .change_context(TmsError::IoError)
            .attach_printable("Failed to set hook permissions")?;
    }

    eprintln!("✓ Created default hook at {}", hook_path.display());
    eprintln!("  Customize it: nvim {}", hook_path.display());

    Ok(())
}

/// Check if a file is executable
#[cfg(unix)]
fn is_executable(path: &PathBuf) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .map(|m| m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(_path: &PathBuf) -> bool {
    true
}

/// Handle creation of a new directory via the create hook
fn create_new_directory(name: &str, config: &tms::configs::Config, tmux: &Tmux) -> Result<()> {
    // Convention: hook is always at ~/.config/tms/create-hook
    let hook_path = dirs::config_dir()
        .ok_or(TmsError::ConfigError)
        .attach_printable("Could not determine config directory")?
        .join("tms/create-hook");

    // Ensure hook exists (create from template if needed)
    ensure_hook_exists(&hook_path)?;

    // Check if executable
    if !is_executable(&hook_path) {
        return Err(TmsError::ConfigError)
            .attach_printable(format!("Hook is not executable: {}", hook_path.display()))
            .attach_printable(format!("Run: chmod +x {}", hook_path.display()));
    }

    // Get search directories from config
    let search_dirs = config
        .search_dirs
        .as_ref()
        .ok_or(TmsError::ConfigError)
        .attach_printable("No search directories configured in config.toml")?;

    let search_paths: Vec<String> = search_dirs
        .iter()
        .map(|d| d.path.to_string_lossy().to_string())
        .collect();

    if search_paths.is_empty() {
        return Err(TmsError::ConfigError).attach_printable("search_dirs is empty in config.toml");
    }

    // Execute hook: create-hook "name" "/path1" "/path2" ...
    // Inherit stderr so user sees progress messages, but capture stdout for directory name
    let output = Command::new(&hook_path)
        .arg(name)
        .args(&search_paths)
        .stderr(std::process::Stdio::inherit())
        .output()
        .change_context(TmsError::IoError)
        .attach_printable("Failed to execute create hook")?;

    // Check exit status
    if !output.status.success() {
        return Err(TmsError::IoError)
            .attach_printable(format!(
                "Hook failed with exit code: {}",
                output.status.code().unwrap_or(-1)
            ))
            .attach_printable("Check hook output for details");
    }

    // Get session name from hook's stdout, or fall back to the typed name
    let hook_output = String::from_utf8_lossy(&output.stdout);
    let session_name = hook_output.trim();
    let session_name = if session_name.is_empty() {
        name
    } else {
        session_name
    };

    // Re-discover sessions to find the one we just created
    let sessions = create_sessions(config)?;
    let session = sessions
        .find_session(session_name)
        .ok_or(TmsError::IoError)
        .attach_printable("Hook did not create a discoverable directory")
        .attach_printable(format!(
            "Expected to find a directory matching: {}",
            session_name
        ))?;

    // Open it using normal session flow
    session.switch_to(tmux, config)?;

    Ok(())
}
