use std::{
    
    collections::{HashSet},

    path::{

        PathBuf,
        Path,
    },
};

use glob::{Pattern};

use actions_toolkit::client::repository::{

    reference::{HandleReference},
    tree::{TreeEntry},
    blob::{Blob},
    sha::{Sha},
};

use anyhow::{Result};

use sha1::{
    
    Digest,
    Sha1,
};

use actions_toolkit::{core as atc};

#[derive(Default, Clone, Debug)]
pub struct CommitOptions<T: AsRef<[Pattern]>> {
    pub(crate) message: String,
    pub(crate) always: Option<bool>,
    pub(crate) source: Option<PathBuf>,
    pub(crate) target: Option<PathBuf>,
    pub(crate) include: Option<T>,
    pub(crate) exclude: Option<T>,
    pub(crate) flatten: Option<bool>,
    pub(crate) force: Option<bool>,
}

impl<T: AsRef<[Pattern]>> CommitOptions<T> {
    pub fn new(message: impl AsRef<str>) -> Result<Self> {
        let message = message.as_ref()
            .to_owned();

        Ok(Self {
            message,
            always: Some(false),
            source: None,
            target: None,
            include: None,
            exclude: None,
            flatten: None,
            force: None,
        })
    }

    pub fn with_always_commit(self, always: Option<bool>) -> Self {
        let CommitOptions { message, source, target, include, exclude, flatten, force, .. } = self;

        CommitOptions { 
            
            message, 
            always, 
            source, 
            target, 
            include,
            exclude,
            flatten,
            force, 
        }
    }

    pub fn with_target_directory(self, target: Option<impl AsRef<Path>>) -> Self {
        let CommitOptions { message, always, source, include, exclude, flatten, force, .. } = self;
        let target = target.map(|path| {
            path.as_ref().to_path_buf()
        });

        CommitOptions { 
            
            message, 
            always, 
            source, 
            target, 
            include,
            exclude,
            flatten,
            force, 
        }
    }

    pub fn with_source_directory(self, source: Option<impl AsRef<Path>>) -> Self {
        let CommitOptions { message, always, target, include, exclude, flatten, force, .. } = self;
        let source = source.map(|path| {
            path.as_ref().to_path_buf()
        });

        CommitOptions { 

            message, 
            always, 
            source, 
            target, 
            include, 
            exclude, 
            flatten, 
            force, 
        }
    }

    pub fn with_flattening(self, flatten: Option<bool>) -> Self {
        let CommitOptions { message, always, source, target, include, exclude, force, .. } = self;

        CommitOptions { 
            
            message, 
            always, 
            source, 
            target, 
            include, 
            exclude, 
            flatten, 
            force, 
        }
    }

    pub fn with_force(self, force: Option<bool>) -> Self {
        let CommitOptions { message, always, source, target, include, exclude, flatten, .. } = self;

        CommitOptions { 
            
            message, 
            always, 
            source, 
            target, 
            include, 
            exclude, 
            flatten, 
            force, 
        }
    }

    pub fn with_include(self, include: Option<T>) -> Self {
        let CommitOptions { message, always, source, target, exclude, flatten, force, .. } = self;

        CommitOptions { 
            
            message, 
            always, 
            source, 
            target, 
            include, 
            exclude, 
            flatten, 
            force, 
        }
    }

    pub fn with_exclude(self, exclude: Option<T>) -> Self {
        let CommitOptions { message, always, source, target, include, flatten, force, .. } = self;

        CommitOptions { 
            
            message, 
            always, 
            source, 
            target, 
            include, 
            exclude, 
            flatten, 
            force, 
        }
    }
}

fn get_file_sha(path: impl AsRef<Path>) -> Result<Sha<'static>> {
    let mut hasher = Sha1::new();

    let prefix = format!("blob {length}\0", length = {
        std::fs::metadata(path.as_ref())?.len()
    });

    let data = std::fs::read({
        path.as_ref()
    })?;

    hasher.update([
        prefix.as_bytes(), 
        data.as_slice()
    ].concat());

    Ok(Sha::from(hex::encode({
        hasher.finalize()
            .as_slice()
    })))
}

fn fetch_unchanged(reference: HandleReference) -> Result<HashSet<PathBuf>> {
    let working_directory = std::env::current_dir()?;

    let repository = reference.get_repository();
    let commit = reference.try_get_commit()?;
    let tree = commit.try_get_tree(false)?;
    
    let mut unchanged = HashSet::new();
    let mut trees = Vec::new();

    for entry in tree.iter().cloned() {
        match entry {
            TreeEntry::Blob { path, sha, .. } => {
                if sha == get_file_sha(working_directory.join(path.as_path()))? {
                    unchanged.insert(path);
                }
            },
            TreeEntry::Tree { path, sha, .. } => {
                trees.push((path.clone(), sha.clone()));
            },
            _ => continue,
        }; 
    }

    use rayon::prelude::*;
    
    let entries: Vec<(PathBuf, Sha<'static>)> = trees.par_iter().filter_map(|(parent, sha)| {
        let tree = repository.try_get_tree(sha.clone(), true).ok()?;
        
        let mut entries = Vec::new();
        for entry in tree.iter().cloned() {
            if let TreeEntry::Blob { path, sha, .. } = entry {
                entries.push((parent.join(path.as_path()), sha));
            }
        }
        
        Some(entries)
    }).flatten()
    .collect();
    
    let entries: HashSet<PathBuf> = entries.par_iter().cloned().filter_map(|(path, sha)| {
        if let Ok(file_sha) = get_file_sha(working_directory.join(path.as_path())) {
            if sha == file_sha { return Some(path) }
        }

        None
    }).collect();

    unchanged.extend(entries.into_iter());

    Ok(unchanged)
}

fn execute<'a, P: AsRef<[Pattern]>>(reference: HandleReference, options: CommitOptions<P>) -> Result<Sha<'static>> {
    let repository = reference.get_repository();
    let base = reference.try_get_commit()?;

    let CommitOptions { 
        message,
        always,
        ref source, 
        ref target, 
        include,
        exclude,
        flatten,
        force,
    } = options;

    let mut entries = HashSet::new();

    let unchanged = fetch_unchanged({
        reference.clone()
    })?;

    if let Some(include) = include {
        for pattern in include.as_ref().iter() {
            for entry in glob::glob(pattern.as_str())?.filter_map(|entry| entry.ok()) {
                if unchanged.contains(entry.as_path()) { continue } else {
                    entries.insert(entry);
                }
            }
        }
    }

    if let Some(exclude) = exclude {
        for pattern in exclude.as_ref().iter() {
            for entry in glob::glob(pattern.as_str())?.filter_map(|entry| entry.ok()) {
                entries.remove(entry.as_path());
            }
        }
    }

    match always {
        Some(false) | None => {
            if entries.is_empty() {
                return Ok(base.get_sha()
                    .to_owned())
            }
        },
        _ => (),
    };

    use rayon::prelude::*;

    let blobs: Vec<Result<Option<(Blob, PathBuf, u32)>>> = {
        entries.par_iter().cloned().map(|mut path| {
            if path.is_symlink() || path.is_dir() {
                return Ok(None)
            }
            
            use std::os::unix::fs::{PermissionsExt};

            let data = std::fs::read(path.as_path())?;
            let mode = std::fs::metadata(path.as_path())?
                .permissions()
                .mode();

            if let Some(source) = source { 
                path = path.strip_prefix(source)?
                    .to_owned()
            }

            if let (Some(true), Some(parent)) = (flatten, path.parent()) {
                path = path.strip_prefix(parent)?
                    .to_owned()
            }

            if let Some(target) = target { 
                path = target.join(path)
            }
            
            let blob = repository.try_create_binary_blob({
                data.as_slice()
            })?;

            Ok(Some((blob, path, mode)))
        }).collect()
    };

    let mut lookup = HashSet::new();
    let mut tree = Vec::new();
    
    for result in blobs {
        let (blob, path, mode) = match result {
            Err(error) => return Err(error),
            Ok(Some(blob)) => blob,
            Ok(None) => continue,
        };

        if let Some(other) = lookup.replace(path.clone()) {
            atc::log::error(format!("Flattening results in conflict for paths: [ '{path}', '{other}' ]", path = path.display(), other = other.display()));
            anyhow::bail!("Flattening results in conflict for paths: [ '{path}', '{other}' ]", path = path.display(), other = other.display());
        }

        tree.push(TreeEntry::Blob { 
            sha: blob.get_sha().to_owned(), path, mode: match mode {
                mode if (mode & 0o111) > 0 => { 0o100755 },
                mode if (mode & 0o444) > 0 => { 0o100644 },
                _ => {
                    
                    atc::log::error(format!("Unsupported mode: '{mode:o}'"));
                    anyhow::bail!("Unsupported mode: '{mode:o}'");
                }
            },
        });
    }

    let tree = if tree.is_empty() { base.try_get_tree(false)? } else {
        repository.try_create_tree_with_base(base.clone(), tree)?
    };

    let commit = {
        repository.try_create_commit([base.clone()], tree, message)?
    };

    reference.try_set_commit(force.unwrap_or(false), {
        commit.clone()
    })?;

    Ok(commit.get_sha()
        .to_owned())
}

pub fn commit<P: AsRef<[Pattern]>>(reference: HandleReference, options: CommitOptions<P>) -> Result<Sha<'static>> {
    let CommitOptions { ref source, .. } = options;

    let workspace = std::env::current_dir()?;

    if let Some(ref source) = source {
        let workspace = std::env::current_dir()?;
        let Ok(source) = workspace.join(source).canonicalize() else {
            atc::log::error(format!("Failed to resolve source path: '{source}'", source = source.display()));
            anyhow::bail!("Failed to resolve source path: '{source}'", source = source.display());
        };

        if !(source.starts_with(workspace)) {
            atc::log::error(format!("Source path is not within workspace: '{source}'", source = source.display()));
            anyhow::bail!("Source path is not within workspace: '{source}'", source = source.display());
        }

        if source.exists() && source.is_dir() { std::env::set_current_dir(source)? } else {
            atc::log::error(format!("Source directory does not exist: '{source}'", source = source.display()));
            anyhow::bail!("Source directory does not exist: '{source}'", source = source.display());
        }
    }

    let result = self::execute(reference, options);

    std::env::set_current_dir({
        workspace
    })?;

    result
}