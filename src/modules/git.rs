use anyhow::{Context, Result};
use git2::{Repository, Signature, BranchType};
use log::debug;
use std::path::Path;

/// Initializes a git repository and makes the first commit on dev branch
pub fn initialize_and_commit(repo_path: &Path, message: &str) -> Result<Repository> {
    debug!("Initializing git repository at {}", repo_path.display());
    
    let repo = if repo_path.join(".git").exists() {
        Repository::open(repo_path)
            .with_context(|| format!("Failed to open existing repository at {}", repo_path.display()))?
    } else {
        let repo = Repository::init(repo_path)
            .with_context(|| format!("Failed to initialize git repository at {}", repo_path.display()))?;
        
        // Set initial branch to dev
        let mut config = repo.config()
            .context("Failed to get repository config")?;
        config.set_str("init.defaultBranch", "dev")
            .context("Failed to set default branch to dev")?;
        
        repo
    };

    // Get the current user's name and email for the commit
    let signature = get_signature(&repo)?;

    // Add all files
    let mut index = repo.index()
        .context("Failed to get repository index")?;
    
    index.add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
        .context("Failed to add files to index")?;
    
    index.write()
        .context("Failed to write index")?;

    let tree_id = index.write_tree()
        .context("Failed to write tree")?;
    
    let tree = repo.find_tree(tree_id)
        .context("Failed to find tree")?;

    // Check if there's already a HEAD commit
    let parent_commit = repo.head().ok().and_then(|head| {
        head.target().and_then(|oid| repo.find_commit(oid).ok())
    });

    // Commit to dev branch
    let commit_id = if let Some(parent) = &parent_commit {
        repo.commit(
            Some("refs/heads/dev"),
            &signature,
            &signature,
            message,
            &tree,
            &[parent],
        )
    } else {
        repo.commit(
            Some("refs/heads/dev"),
            &signature,
            &signature,
            message,
            &tree,
            &[],
        )
    }
    .context("Failed to create commit")?;

    // Set HEAD to dev branch
    repo.set_head("refs/heads/dev")
        .context("Failed to set HEAD to dev branch")?;

    debug!("Created initial commit on dev branch: {}", commit_id);
    
    // Drop tree and parent_commit to release borrows before returning repo
    drop(tree);
    drop(parent_commit);
    Ok(repo)
}

/// Creates a new branch from the current HEAD
pub fn create_branch(repo: &Repository, branch_name: &str) -> Result<()> {
    debug!("Creating branch: {}", branch_name);
    
    let head = repo.head()
        .context("Failed to get HEAD")?;
    
    let head_commit = head.target()
        .and_then(|oid| repo.find_commit(oid).ok())
        .context("Failed to find HEAD commit")?;

    let _branch = repo.branch(branch_name, &head_commit, false)
        .with_context(|| format!("Failed to create branch '{}'", branch_name))?;

    debug!("Created branch: {}", branch_name);
    Ok(())
}

/// Pushes the current branch to the remote using git CLI (handles authentication better)
pub fn push_to_remote(repo: &Repository, remote_name: &str, branch_name: &str, set_upstream: bool) -> Result<()> {
    debug!("Pushing branch '{}' to remote '{}'", branch_name, remote_name);
    
    let repo_path = repo.path().parent()
        .ok_or_else(|| anyhow::anyhow!("Invalid repository path"))?;

    // Use git CLI for push as it handles authentication better
    let mut cmd = std::process::Command::new("git");
    cmd.arg("push");
    if set_upstream {
        cmd.arg("-u");
    }
    cmd.arg(remote_name)
        .arg(branch_name)
        .current_dir(repo_path);
    
    let output = cmd.output()
        .with_context(|| format!("Failed to push branch '{}' to remote", branch_name))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("Failed to push branch '{}': {}", branch_name, stderr));
    }

    debug!("Successfully pushed branch '{}'", branch_name);
    Ok(())
}

/// Sets the upstream branch for the current branch
pub fn set_upstream_branch(repo: &Repository, branch_name: &str, remote_name: &str) -> Result<()> {
    debug!("Setting upstream for branch '{}' to '{}/{}'", branch_name, remote_name, branch_name);
    
    let mut branch = repo.find_branch(branch_name, BranchType::Local)
        .with_context(|| format!("Failed to find branch '{}'", branch_name))?;

    let upstream_name = format!("{}/{}", remote_name, branch_name);
    branch.set_upstream(Some(&upstream_name))
        .with_context(|| format!("Failed to set upstream for branch '{}'", branch_name))?;

    Ok(())
}

/// Gets the signature for commits (user name and email)
pub fn get_signature(repo: &Repository) -> Result<Signature<'static>> {
    // Try to get from git config
    let config = repo.config()
        .context("Failed to get repository config")?;

    let name = config.get_string("user.name")
        .unwrap_or_else(|_| "NCL User".to_string());
    
    let email = config.get_string("user.email")
        .unwrap_or_else(|_| "ncl@example.com".to_string());

    Signature::now(&name, &email)
        .context("Failed to create signature")
}

/// Checks out a branch
pub fn checkout_branch(repo: &Repository, branch_name: &str) -> Result<()> {
    debug!("Checking out branch: {}", branch_name);
    
    let (object, reference) = repo.revparse_ext(branch_name)
        .with_context(|| format!("Failed to parse branch '{}'", branch_name))?;

    repo.checkout_tree(&object, None)
        .with_context(|| format!("Failed to checkout tree for branch '{}'", branch_name))?;

    match reference {
        Some(ref r) => {
            repo.set_head(r.name().unwrap())
                .with_context(|| format!("Failed to set HEAD to '{}'", branch_name))?;
        }
        None => {
            // Create a new branch reference
            repo.reference(
                &format!("refs/heads/{}", branch_name),
                object.id(),
                true,
                "checkout branch",
            )
            .with_context(|| format!("Failed to create branch reference '{}'", branch_name))?;
            
            repo.set_head(&format!("refs/heads/{}", branch_name))
                .with_context(|| format!("Failed to set HEAD to '{}'", branch_name))?;
        }
    }

    debug!("Checked out branch: {}", branch_name);
    Ok(())
}

