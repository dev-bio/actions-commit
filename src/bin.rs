use actions_toolkit::{core as atc};

use anyhow::{Result};
use glob::{Pattern};

use actions_toolkit::client::client::{Client};
use actions_commit::{CommitOptions};

fn main() -> Result<()> {
    let Some(repository) = atc::get_input("github-repository") else {
        atc::log::error("Missing input 'github-repository'!");
        anyhow::bail!("Missing input 'github-repository'!");
    };

    let Some(reference) = atc::get_input("github-reference") else {
        atc::log::error("Missing input 'github-reference'!");
        anyhow::bail!("Missing input 'github-reference'!");
    };

    let Some(message) = atc::get_input("message") else {
        atc::log::error("Missing input 'message'!");
        anyhow::bail!("Missing input 'message'!");
    };

    let include: Option<Vec<Pattern>> = atc::get_multiline_input("include").and_then(|lines| {
        Some(lines.iter().filter_map(|line| {
            Pattern::new(line).ok()
        }).collect())
    });

    let exclude: Option<Vec<Pattern>> = atc::get_multiline_input("exclude").and_then(|lines| {
        Some(lines.iter().filter_map(|line| {
            Pattern::new(line).ok()
        }).collect())
    });

    let client = Client::new_with_token({
        atc::get_input("github-token")
    })?;
    
    let account = client.try_get_account({
        repository.as_str()
    })?;
    
    let repository = account.try_get_repository({
        repository.as_str()
    })?;

    let reference = repository.try_get_reference(reference)?;
    let result = actions_commit::commit(reference, CommitOptions::new(message)?
        .with_flattening(atc::get_boolean_input("flatten"))
        .with_force(atc::get_boolean_input("force"))
        .with_source_directory(atc::get_input("source"))
        .with_target_directory(atc::get_input("target"))
        .with_include(include)
        .with_exclude(exclude));

    if let Err(error) = result {
        atc::log::error(format!("{error}"));
        anyhow::bail!(error);
    }

    Ok(())
}