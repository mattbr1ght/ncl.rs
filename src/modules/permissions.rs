use anyhow::Result;
use log::debug;
use std::path::Path;
use std::process::Command;

/// Fixes file ownership recursively to ensure all files are owned by the current user
/// This is important because Docker containers might create root-owned files
/// This is a best-effort operation - if files are root-owned and chown fails, it will log but not fail
pub fn fix_file_ownership(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        // Get current user ID and group ID
        let uid = match get_current_uid() {
            Some(uid) => uid,
            None => {
                debug!("Could not get UID, skipping ownership fix");
                return Ok(());
            }
        };
        let gid = match get_current_gid() {
            Some(gid) => gid,
            None => {
                debug!("Could not get GID, skipping ownership fix");
                return Ok(());
            }
        };

        debug!(
            "Fixing ownership for {} (uid: {}, gid: {})",
            path.display(),
            uid,
            gid
        );

        // Try chown with numeric UID:GID first (more reliable)
        let output = Command::new("chown")
            .arg("-R")
            .arg(format!("{}:{}", uid, gid))
            .arg(path)
            .output();

        match output {
            Ok(output) if output.status.success() => {
                debug!("Successfully fixed file ownership");
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                // If chown fails due to permission denied (root-owned files),
                // try with username as fallback, but don't fail
                debug!("chown with UID:GID failed: {}", stderr);

                // Try with username as fallback
                let username = whoami::username();
                if let Ok(output) = Command::new("chown")
                    .arg("-R")
                    .arg(format!("{}:{}", username, username))
                    .arg(path)
                    .output()
                {
                    if !output.status.success() {
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        debug!("chown with username also failed: {}", stderr);
                        debug!(
                            "Note: Some files may be root-owned. You may need to run: sudo chown -R $USER:$USER {}",
                            path.display()
                        );
                    }
                }
            }
            Err(e) => {
                debug!("Failed to execute chown: {}", e);
                debug!(
                    "Note: Some files may be root-owned. You may need to run: sudo chown -R $USER:$USER {}",
                    path.display()
                );
            }
        }

        // Also ensure files are writable (this should always work for files we own)
        fix_file_permissions(path)?;
    }

    #[cfg(windows)]
    {
        // On Windows, we just ensure files are writable
        // Ownership is less of an issue on Windows
        fix_file_permissions(path)?;
    }

    Ok(())
}

#[cfg(unix)]
fn get_current_uid() -> Option<String> {
    Command::new("id")
        .arg("-u")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
}

#[cfg(unix)]
fn get_current_gid() -> Option<String> {
    Command::new("id")
        .arg("-g")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
}

/// Fixes file permissions to ensure files are writable by the current user
fn fix_file_permissions(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::fs;
        use std::os::unix::fs::PermissionsExt;

        for entry in walkdir::WalkDir::new(path) {
            let entry = entry?;
            let path = entry.path();

            if let Ok(metadata) = fs::metadata(path) {
                let mut perms = metadata.permissions();

                if path.is_file() {
                    // Set to rw-rw-r-- (664) for files
                    perms.set_mode(0o664);
                } else if path.is_dir() {
                    // Set to rwxrwxr-x (775) for directories
                    perms.set_mode(0o775);
                }

                if fs::set_permissions(path, perms).is_err() {
                    debug!("Failed to set permissions for {}", path.display());
                    // Continue - don't fail on permission errors
                }
            }
        }
    }

    #[cfg(windows)]
    {
        // On Windows, ensure files are not read-only
        for entry in walkdir::WalkDir::new(path) {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() {
                if let Ok(mut perms) = std::fs::metadata(path).map(|m| m.permissions()) {
                    perms.set_readonly(false);
                    if std::fs::set_permissions(path, perms).is_err() {
                        debug!("Failed to set permissions for {}", path.display());
                        // Continue - don't fail on permission errors
                    }
                }
            }
        }
    }

    Ok(())
}
