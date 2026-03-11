use aho_corasick::{AhoCorasickBuilder, MatchKind};
use error_stack::{report, Report, ResultExt};
use gix::Repository;
use std::{
    collections::{HashMap, VecDeque},
    fs::{self},
    path::{Path, PathBuf},
    process::{self, Stdio},
};

use crate::{
    configs::{Config, SearchDirectory},
    dirty_paths::DirtyUtf8Path,
    session::{Session, SessionType},
    Result, TmsError,
};

pub trait Worktree {
    fn name(&self) -> String;

    fn path(&self) -> Result<PathBuf>;

    fn is_prunable(&self) -> bool;
}

impl Worktree for gix::worktree::Proxy<'_> {
    fn name(&self) -> String {
        self.id().to_string()
    }

    fn path(&self) -> Result<PathBuf> {
        self.base().change_context(TmsError::GitError)
    }

    fn is_prunable(&self) -> bool {
        !self.base().is_ok_and(|path| path.exists())
    }
}

pub enum RepoProvider {
    Git(Box<Repository>),
}

impl From<gix::Repository> for RepoProvider {
    fn from(repo: gix::Repository) -> Self {
        Self::Git(Box::new(repo))
    }
}

impl RepoProvider {
    pub fn open(path: &Path, _config: &Config) -> Result<Self> {
        gix::open(path)
            .map(|repo| RepoProvider::Git(Box::new(repo)))
            .change_context(TmsError::GitError)
    }

    pub fn is_worktree(&self) -> bool {
        match self {
            RepoProvider::Git(repo) => !repo.main_repo().is_ok_and(|r| r == **repo),
        }
    }

    pub fn path(&self) -> &Path {
        match self {
            RepoProvider::Git(repo) => repo.path(),
        }
    }

    pub fn main_repo(&self) -> Option<PathBuf> {
        match self {
            RepoProvider::Git(repo) => repo.main_repo().map(|repo| repo.path().to_path_buf()).ok(),
        }
    }

    pub fn work_dir(&self) -> Option<&Path> {
        match self {
            RepoProvider::Git(repo) => repo.workdir(),
        }
    }

    pub fn head_name(&self) -> Result<String> {
        match self {
            RepoProvider::Git(repo) => Ok(repo
                .head_name()
                .change_context(TmsError::GitError)?
                .ok_or(TmsError::GitError)?
                .shorten()
                .to_string()),
        }
    }

    pub fn is_bare(&self) -> bool {
        match self {
            RepoProvider::Git(repo) => repo.is_bare(),
        }
    }

    pub fn add_worktree(&self, path: &Path) -> Result<Option<(String, PathBuf)>> {
        match self {
            RepoProvider::Git(_) => {
                let Ok(head) = self.head_name() else {
                    return Ok(None);
                };
                // Add the default branch as a tree (usually either main or master)
                process::Command::new("git")
                    .current_dir(path)
                    .args(["worktree", "add", &head])
                    .stderr(Stdio::inherit())
                    .output()
                    .change_context(TmsError::GitError)?;
                Ok(Some((head.clone(), path.to_path_buf().join(&head))))
            }
        }
    }

    pub fn worktrees(&'_ self, _config: &Config) -> Result<Vec<Box<dyn Worktree + '_>>> {
        match self {
            RepoProvider::Git(repo) => Ok(repo
                .worktrees()
                .change_context(TmsError::GitError)?
                .into_iter()
                .map(|i| Box::new(i) as Box<dyn Worktree>)
                .collect()),
        }
    }
}

pub fn find_repos(config: &Config) -> Result<HashMap<String, Vec<Session>>> {
    let mut repos: HashMap<String, Vec<Session>> = HashMap::new();

    search_dirs(config, |file, repo| {
        if repo.is_worktree() {
            return Ok(());
        }

        let session_name = file
            .path
            .file_name()
            .ok_or_else(|| {
                Report::new(TmsError::GitError).attach_printable("Not a valid repository name")
            })?
            .to_string()?;

        let session = Session::new(session_name, SessionType::Git(repo));
        if let Some(list) = repos.get_mut(&session.name) {
            list.push(session);
        } else {
            repos.insert(session.name.clone(), vec![session]);
        }
        Ok(())
    })?;
    Ok(repos)
}

fn search_dirs<F>(config: &Config, mut f: F) -> Result<()>
where
    F: FnMut(SearchDirectory, RepoProvider) -> Result<()>,
{
    {
        let directories = config.search_dirs().change_context(TmsError::ConfigError)?;
        let mut to_search: VecDeque<SearchDirectory> = directories.into();

        let excluder = if let Some(excluded_dirs) = &config.excluded_dirs {
            Some(
                AhoCorasickBuilder::new()
                    .match_kind(MatchKind::LeftmostFirst)
                    .build(excluded_dirs)
                    .change_context(TmsError::IoError)?,
            )
        } else {
            None
        };

        while let Some(file) = to_search.pop_front() {
            if let Some(ref excluder) = excluder {
                if excluder.is_match(&file.path.to_string()?) {
                    continue;
                }
            }

            if let Ok(repo) = RepoProvider::open(&file.path, config) {
                f(file, repo)?;
            } else if file.path.is_dir() && file.depth > 0 {
                match fs::read_dir(&file.path) {
                    Err(ref e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                        eprintln!(
                        "Warning: insufficient permissions to read '{0}'. Skipping directory...",
                        file.path.to_string()?
                    );
                    }
                    Err(e) => {
                        let report = report!(e)
                            .change_context(TmsError::IoError)
                            .attach_printable(format!("Could not read directory {:?}", file.path));
                        return Err(report);
                    }
                    Ok(read_dir) => {
                        let mut subdirs = read_dir
                            .filter_map(|dir_entry| {
                                if let Ok(dir) = dir_entry {
                                    Some(SearchDirectory::new(dir.path(), file.depth - 1))
                                } else {
                                    None
                                }
                            })
                            .collect::<VecDeque<SearchDirectory>>();

                        if !subdirs.is_empty() {
                            to_search.append(&mut subdirs);
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

