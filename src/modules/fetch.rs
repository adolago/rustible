//! Fetch module — copy a file from the target to the control node.
//!
//! This is the counterpart of `copy`: it reads a file on the target and writes
//! it under a local destination directory, the way Ansible's `fetch` gathers
//! logs or configuration from a fleet.

use super::{
    Module, ModuleClassification, ModuleContext, ModuleError, ModuleOutput, ModuleParams,
    ModuleResult, ParamExt,
};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

/// Module for fetching files from a target
pub struct FetchModule;

impl FetchModule {
    /// Where the fetched file is written.
    ///
    /// With `flat: true` the destination is used as given (a trailing slash
    /// means "into this directory"); otherwise Ansible's layout is used:
    /// `<dest>/<host>/<remote path>`.
    fn destination(dest: &str, host: &str, source: &str, flat: bool) -> PathBuf {
        if flat {
            let dest_path = PathBuf::from(dest);
            if dest.ends_with('/') || dest_path.is_dir() {
                let name = Path::new(source)
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "fetched".to_string());
                return dest_path.join(name);
            }
            return dest_path;
        }

        let relative = source.trim_start_matches('/');
        PathBuf::from(dest).join(host).join(relative)
    }

    fn checksum(bytes: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        format!("{:x}", hasher.finalize())
    }
}

impl Module for FetchModule {
    fn name(&self) -> &'static str {
        "fetch"
    }

    fn description(&self) -> &'static str {
        "Fetch a file from the target to the control node"
    }

    fn classification(&self) -> ModuleClassification {
        ModuleClassification::NativeTransport
    }

    fn required_params(&self) -> &[&'static str] {
        &["src", "dest"]
    }

    fn execute(
        &self,
        params: &ModuleParams,
        context: &ModuleContext,
    ) -> ModuleResult<ModuleOutput> {
        let src = params.get_string_required("src")?;
        let dest = params.get_string_required("dest")?;
        let flat = params.get_bool_or("flat", false);
        let fail_on_missing = params.get_bool_or("fail_on_missing", true);

        let host = context
            .vars
            .get("inventory_hostname")
            .and_then(|value| value.as_str())
            .unwrap_or("localhost")
            .to_string();
        let target = Self::destination(&dest, &host, &src, flat);

        // Read the source: through the connection for a remote target, from
        // the filesystem when running on the control node.
        let bytes = match &context.connection {
            Some(connection) if !connection.is_local() => {
                let connection = connection.clone();
                let source = src.clone();
                super::block_on_module_future(async move {
                    connection.download_content(Path::new(&source)).await
                })?
                .map_err(|error| {
                    ModuleError::ExecutionFailed(format!("Failed to fetch '{}': {}", src, error))
                })
            }
            _ => fs::read(&src).map_err(|error| {
                ModuleError::ExecutionFailed(format!("Failed to read '{}': {}", src, error))
            }),
        };

        let bytes = match bytes {
            Ok(bytes) => bytes,
            Err(error) => {
                if fail_on_missing {
                    return Err(error);
                }
                return Ok(ModuleOutput::ok(format!(
                    "Skipped missing source '{}'",
                    src
                )));
            }
        };

        let checksum = Self::checksum(&bytes);

        // Nothing to do when the local copy already matches.
        if let Ok(existing) = fs::read(&target) {
            if Self::checksum(&existing) == checksum {
                return Ok(ModuleOutput::ok(format!(
                    "'{}' already matches the target",
                    target.display()
                ))
                .with_data("dest", serde_json::json!(target.to_string_lossy()))
                .with_data("checksum", serde_json::json!(checksum)));
            }
        }

        if context.check_mode {
            return Ok(ModuleOutput::changed(format!(
                "Would fetch '{}' to '{}'",
                src,
                target.display()
            ))
            .with_data("dest", serde_json::json!(target.to_string_lossy())));
        }

        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&target, &bytes)?;

        Ok(
            ModuleOutput::changed(format!("Fetched '{}' to '{}'", src, target.display()))
                .with_data("dest", serde_json::json!(target.to_string_lossy()))
                .with_data("checksum", serde_json::json!(checksum)),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn params(pairs: &[(&str, serde_json::Value)]) -> ModuleParams {
        let mut map: HashMap<String, serde_json::Value> = HashMap::new();
        for (key, value) in pairs {
            map.insert((*key).to_string(), value.clone());
        }
        map
    }

    #[test]
    fn test_destination_uses_ansible_layout() {
        let path = FetchModule::destination("/backup", "web1", "/etc/hosts", false);
        assert_eq!(path, PathBuf::from("/backup/web1/etc/hosts"));
    }

    #[test]
    fn test_flat_destination_keeps_the_given_path() {
        let path = FetchModule::destination("/backup/hosts.txt", "web1", "/etc/hosts", true);
        assert_eq!(path, PathBuf::from("/backup/hosts.txt"));
    }

    #[test]
    fn test_flat_destination_directory_keeps_the_file_name() {
        let path = FetchModule::destination("/backup/", "web1", "/etc/hosts", true);
        assert_eq!(path, PathBuf::from("/backup/hosts"));
    }

    #[test]
    fn test_fetches_a_local_file() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.txt");
        let dest = dir.path().join("collected");
        fs::write(&source, "payload\n").unwrap();

        let module = FetchModule;
        let output = module
            .execute(
                &params(&[
                    ("src", serde_json::json!(source.to_str().unwrap())),
                    ("dest", serde_json::json!(dest.to_str().unwrap())),
                    ("flat", serde_json::json!(true)),
                ]),
                &ModuleContext::default(),
            )
            .unwrap();

        assert!(output.changed);
        assert_eq!(fs::read_to_string(&dest).unwrap(), "payload\n");
    }

    #[test]
    fn test_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source.txt");
        let dest = dir.path().join("collected");
        fs::write(&source, "payload\n").unwrap();
        fs::write(&dest, "payload\n").unwrap();

        let module = FetchModule;
        let output = module
            .execute(
                &params(&[
                    ("src", serde_json::json!(source.to_str().unwrap())),
                    ("dest", serde_json::json!(dest.to_str().unwrap())),
                    ("flat", serde_json::json!(true)),
                ]),
                &ModuleContext::default(),
            )
            .unwrap();

        assert!(!output.changed, "an identical copy is not a change");
    }

    #[test]
    fn test_missing_source_can_be_tolerated() {
        let dir = tempfile::tempdir().unwrap();
        let module = FetchModule;

        let error = module
            .execute(
                &params(&[
                    ("src", serde_json::json!("/nonexistent/rustible-fetch")),
                    ("dest", serde_json::json!(dir.path().to_str().unwrap())),
                ]),
                &ModuleContext::default(),
            )
            .unwrap_err();
        assert!(error.to_string().contains("Failed to read"));

        let output = module
            .execute(
                &params(&[
                    ("src", serde_json::json!("/nonexistent/rustible-fetch")),
                    ("dest", serde_json::json!(dir.path().to_str().unwrap())),
                    ("fail_on_missing", serde_json::json!(false)),
                ]),
                &ModuleContext::default(),
            )
            .unwrap();
        assert!(!output.changed);
    }
}
