use std::{
    
    collections::{HashSet},

    path::{

        PathBuf,
        Path,
    },
};

use glob::{Pattern};

use actions_toolkit::client::{

    repository::{

        reference::{HandleReference}, 
        tree::{TreeEntry},
        sha::{Sha},
    },
};

use anyhow::{Result};

use actions_toolkit::{core as atc};

#[derive(Default, Clone, Debug)]
pub struct CommitOptions<T: AsRef<[Pattern]>> {
    pub(crate) message: String,
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
            source: None,
            target: None,
            include: None,
            exclude: None,
            flatten: None,
            force: None,
        })
    }

    pub fn with_target_directory(self, target: Option<impl AsRef<Path>>) -> Self {
        let CommitOptions { message, source, include, exclude, flatten, force, .. } = self;
        let target = target.map(|path| {
            path.as_ref().to_path_buf()
        });

        CommitOptions { 
            
            message,
            source, 
            target, 
            include,
            exclude,
            flatten,
            force,
        }
    }

    pub fn with_source_directory(self, source: Option<impl AsRef<Path>>) -> Self {
        let CommitOptions { message, target, include, exclude, flatten, force, .. } = self;
        let source = source.map(|path| {
            path.as_ref().to_path_buf()
        });

        CommitOptions { 

            message,
            source, 
            target, 
            include,
            exclude,
            flatten,
            force,
        }
    }

    pub fn with_flattening(self, flatten: Option<bool>) -> Self {
        let CommitOptions { message, source, target, include, exclude, force, .. } = self;

        CommitOptions { 
            
            message,
            source, 
            target, 
            include,
            exclude,
            flatten,
            force,
        }
    }

    pub fn with_force(self, force: Option<bool>) -> Self {
        let CommitOptions { message, source, target, include, exclude, flatten, .. } = self;

        CommitOptions { 
            
            message,
            source, 
            target, 
            include,
            exclude,
            flatten,
            force,
        }
    }

    pub fn with_include(self, include: Option<T>) -> Self {
        let CommitOptions { message, source, target, exclude, flatten, force, .. } = self;

        CommitOptions { 
            
            message,
            source, 
            target, 
            include,
            exclude,
            flatten,
            force,
        }
    }

    pub fn with_exclude(self, exclude: Option<T>) -> Self {
        let CommitOptions { message, source, target, include, flatten, force, .. } = self;

        CommitOptions { 
            
            message,
            source, 
            target, 
            include,
            exclude,
            flatten,
            force,
        }
    }
}

fn execute<'a, P: AsRef<[Pattern]>>(reference: HandleReference, options: CommitOptions<P>) -> Result<Sha<'static>> {
    let repository = reference.get_repository();
    let base = reference.try_get_commit()?;

    let CommitOptions { 
        message,
        ref source, 
        ref target, 
        include,
        exclude,
        flatten,
        force,
    } = options;

    let mut entries = HashSet::new();

    if let Some(include) = include {
        for pattern in include.as_ref().iter() {
            for entry in glob::glob(pattern.as_str())?.filter_map(|entry| entry.ok()) {
                entries.insert(entry);
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
    
    let mut lookup = HashSet::new();
    let mut blobs = Vec::new();

    for mut path in entries {
        if path.is_dir() {
            continue
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

        if let Some(other) = lookup.replace(path.clone()) {
            atc::log::error(format!("Flattening results in conflict for paths: [ '{path}', '{other}' ]", path = path.display(), other = other.display()));
            anyhow::bail!("Flattening results in conflict for paths: [ '{path}', '{other}' ]", path = path.display(), other = other.display());
        }

        atc::log::debug("Creating binary blob!");
        
        let Ok(blob) = repository.try_create_binary_blob(data.as_slice()) else {
            atc::log::error(format!("Failed creating blob for path: '{path}'", path = path.display()));
            anyhow::bail!("Failed creating blob for path: '{path}'", path = path.display());
        };

        atc::log::debug("Created blob: '{sha}'");

        blobs.push(TreeEntry::Blob { 
            sha: blob.get_sha().to_owned(), path, mode: {
                (mode & 0xFFFFFFED) | 0o100000
            },
        });
    }

    let tree = if blobs.is_empty() { base.try_get_tree(false)? } else {
        repository.try_create_tree_with_base(base.clone(), blobs)?
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