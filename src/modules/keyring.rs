use anyhow::Result;
use log::debug;

#[cfg(feature = "keyring-support")]
use anyhow::Context;
#[cfg(feature = "keyring-support")]
use keyring::Entry;

const SERVICE_NAME: &str = "ncl-cli";

/// Keyring wrapper for cross-platform secret storage
pub struct Keyring;

impl Keyring {
    /// Returns true when the keyring backend is available
    pub fn is_available() -> bool {
        cfg!(feature = "keyring-support")
    }

    /// Get a secret from the keyring
    pub fn get(account: &str) -> Result<Option<String>> {
        #[cfg(feature = "keyring-support")]
        {
            let entry = Entry::new(SERVICE_NAME, account).context("Failed to access keyring")?;

            match entry.get_password() {
                Ok(password) => {
                    if password.is_empty() {
                        Ok(None)
                    } else {
                        Ok(Some(password))
                    }
                }
                Err(keyring::Error::NoEntry) => Ok(None),
                Err(e) => Err(anyhow::anyhow!("Keyring error: {}", e)),
            }
        }

        #[cfg(not(feature = "keyring-support"))]
        {
            let _ = account;
            Ok(None)
        }
    }

    /// Set a secret in the keyring
    pub fn set(account: &str, secret: &str) -> Result<()> {
        #[cfg(feature = "keyring-support")]
        {
            let entry = Entry::new(SERVICE_NAME, account).context("Failed to access keyring")?;

            entry
                .set_password(secret)
                .map_err(|e| anyhow::anyhow!("Failed to store secret in keyring: {}", e))
        }

        #[cfg(not(feature = "keyring-support"))]
        {
            let _ = (account, secret);
            Ok(())
        }
    }

    /// Delete a secret from the keyring
    /// Note: keyring v3 may not support deletion, so this is best-effort
    pub fn delete(account: &str) -> Result<()> {
        #[cfg(feature = "keyring-support")]
        {
            // For keyring v3, deletion may not be directly supported
            // We'll attempt to set an empty password as a workaround
            let entry = Entry::new(SERVICE_NAME, account).context("Failed to access keyring")?;

            // Best effort: try to set empty password (some keyrings don't support this)
            // If it fails, we just log and continue
            match entry.set_password("") {
                Ok(()) => {
                    debug!("Cleared keyring entry for {}", account);
                    Ok(())
                }
                Err(e) => {
                    debug!(
                        "Could not clear keyring entry for {}: {} (this is non-critical)",
                        account, e
                    );
                    Ok(()) // Best effort, non-critical
                }
            }
        }

        #[cfg(not(feature = "keyring-support"))]
        {
            let _ = account;
            Ok(())
        }
    }

    /// Check if a secret exists in the keyring
    pub fn exists(account: &str) -> bool {
        #[cfg(feature = "keyring-support")]
        {
            Self::get(account).is_ok_and(|opt| opt.is_some())
        }

        #[cfg(not(feature = "keyring-support"))]
        {
            let _ = account;
            false
        }
    }
}

/// Account names for different secrets
pub mod accounts {
    pub const GITHUB_TOKEN: &str = "github_token";
    pub const COOLIFY_TOKEN: &str = "coolify_token";
    pub const REGISTRY_TOKEN: &str = "registry_token";
    pub const REGISTRY_USER: &str = "registry_user";
}
