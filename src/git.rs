use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Clone a repository using git
pub fn clone(url: &str, args: &[String]) -> Result<()> {
    let mut cmd = Command::new("git");
    cmd.arg("clone").arg(url);
    
    for arg in args {
        cmd.arg(arg);
    }
    
    let status = cmd
        .status()
        .context("Failed to execute git clone")?;
    
    if !status.success() {
        anyhow::bail!("Git clone failed with status: {}", status);
    }
    
    Ok(())
}

/// Create a remote repository on an agito server via SSH
pub fn create_remote_repo(server: &str, user: &str, repo_name: &str) -> Result<()> {
    let repo_name = if !repo_name.ends_with(".git") {
        format!("{}.git", repo_name)
    } else {
        repo_name.to_string()
    };
    
    // Parse server and port
    let (host, port) = if let Some(idx) = server.find(':') {
        let (h, p) = server.split_at(idx);
        (h, &p[1..])
    } else {
        (server, "22")
    };
    
    // SSH command to create repository on server
    let ssh_cmd = format!("agito-create-repo {}", repo_name);
    let status = Command::new("ssh")
        .arg("-p")
        .arg(port)
        .arg(format!("{}@{}", user, host))
        .arg(ssh_cmd)
        .status()
        .context("Failed to execute ssh command")?;
    
    if !status.success() {
        anyhow::bail!("Failed to create remote repository");
    }
    
    Ok(())
}

/// Initialize a bare git repository
pub fn init_bare_repo(path: &Path) -> Result<()> {
    fs::create_dir_all(path)
        .context("Failed to create directory")?;
    
    let output = Command::new("git")
        .arg("init")
        .arg("--bare")
        .arg(path)
        .output()
        .context("Failed to init repository")?;
    
    if !output.status.success() {
        anyhow::bail!(
            "Failed to init repository: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    
    // Set up default hooks
    setup_hooks(path)?;
    
    Ok(())
}

/// Set up server-side git hooks
fn setup_hooks(repo_path: &Path) -> Result<()> {
    let hooks_dir = repo_path.join("hooks");
    
    // Create post-receive hook for CI/CD
    let post_receive = hooks_dir.join("post-receive");
    let post_receive_content = r#"#!/bin/sh
# Agito post-receive hook
# This hook is called after a push is completed

echo "Running post-receive hook..."

# Read the pushed refs
while read oldrev newrev refname; do
    echo "Processing: $refname"
    echo "  Old: $oldrev"
    echo "  New: $newrev"
    
    # Extract branch name
    branch=$(echo $refname | sed 's/refs\/heads\///')
    
    # Run CI/CD if configured
    if [ -f "$GIT_DIR/agito-ci.sh" ]; then
        echo "Running CI/CD pipeline for branch: $branch"
        sh "$GIT_DIR/agito-ci.sh" "$branch" "$oldrev" "$newrev"
    fi
done

echo "Post-receive hook completed."
"#;
    fs::write(&post_receive, post_receive_content)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&post_receive, fs::Permissions::from_mode(0o755))?;
    }
    
    // Create pre-receive hook for validation
    let pre_receive = hooks_dir.join("pre-receive");
    let pre_receive_content = r#"#!/bin/sh
# Agito pre-receive hook
# This hook is called before a push is accepted

echo "Running pre-receive hook..."

# Read the refs being pushed
while read oldrev newrev refname; do
    echo "Validating: $refname"
    
    # Add custom validation logic here
    # Return non-zero to reject the push
done

echo "Pre-receive validation passed."
exit 0
"#;
    fs::write(&pre_receive, pre_receive_content)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&pre_receive, fs::Permissions::from_mode(0o755))?;
    }
    
    // Create update hook
    let update = hooks_dir.join("update");
    let update_content = r#"#!/bin/sh
# Agito update hook
# This hook is called for each ref being updated

refname="$1"
oldrev="$2"
newrev="$3"

echo "Update hook: $refname"

# Add custom branch protection logic here
# Return non-zero to reject the update

exit 0
"#;
    fs::write(&update, update_content)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&update, fs::Permissions::from_mode(0o755))?;
    }
    
    Ok(())
}

/// Get repository information
pub fn get_repo_info(repo_path: &Path) -> Result<std::collections::HashMap<String, String>> {
    let mut info = std::collections::HashMap::new();
    
    // Get description
    let desc_path = repo_path.join("description");
    if let Ok(desc) = fs::read_to_string(&desc_path) {
        info.insert("description".to_string(), desc.trim().to_string());
    }
    
    // Check if it's a bare repo
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_path)
        .arg("rev-parse")
        .arg("--is-bare-repository")
        .output();
    
    if let Ok(output) = output {
        if output.status.success() {
            info.insert(
                "bare".to_string(),
                String::from_utf8_lossy(&output.stdout).trim().to_string(),
            );
        }
    }
    
    Ok(info)
}

/// List all refs in a repository
pub fn list_refs(repo_path: &Path) -> Result<Vec<String>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_path)
        .arg("show-ref")
        .output();
    
    match output {
        Ok(output) if output.status.success() => {
            let refs = String::from_utf8_lossy(&output.stdout)
                .lines()
                .map(|s| s.to_string())
                .collect();
            Ok(refs)
        }
        _ => {
            // Empty repository might not have refs yet
            Ok(vec![])
        }
    }
}
