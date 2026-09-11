//! Slurp module — read a file from the target into a variable.
//!
//! Returns the file's content base64-encoded under `content`, as Ansible's
//! `slurp` does, so a playbook can register it and decode with `b64decode`.

use super::{
    Module, ModuleClassification, ModuleContext, ModuleError, ModuleOutput, ModuleParams,
    ModuleResult, ParamExt,
};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use std::fs;
use std::path::Path;

/// Module for reading remote files into variables
pub struct SlurpModule;

impl Module for SlurpModule {
    fn name(&self) -> &'static str {
        "slurp"
    }

    fn description(&self) -> &'static str {
        "Read a file from the target into a base64-encoded variable"
    }

    fn classification(&self) -> ModuleClassification {
        ModuleClassification::NativeTransport
    }

    fn required_params(&self) -> &[&'static str] {
        &["src"]
    }

    fn execute(
        &self,
        params: &ModuleParams,
        context: &ModuleContext,
    ) -> ModuleResult<ModuleOutput> {
        // Ansible accepts both `src` and `path`.
        let source = match params.get_string("src")? {
            Some(source) => source,
            None => params.get_string_required("path")?,
        };

        let bytes = match &context.connection {
            Some(connection) if !connection.is_local() => {
                let connection = connection.clone();
                let path = source.clone();
                super::block_on_module_future(async move {
                    connection.download_content(Path::new(&path)).await
                })?
                .map_err(|error| {
                    ModuleError::ExecutionFailed(format!("Failed to read '{}': {}", source, error))
                })?
            }
            _ => fs::read(&source).map_err(|error| {
                ModuleError::ExecutionFailed(format!("Failed to read '{}': {}", source, error))
            })?,
        };

        // Reading a file changes nothing, so the result is always ok.
        Ok(
            ModuleOutput::ok(format!("Read {} byte(s) from '{}'", bytes.len(), source))
                .with_data("content", serde_json::json!(STANDARD.encode(&bytes)))
                .with_data("encoding", serde_json::json!("base64"))
                .with_data("source", serde_json::json!(source)),
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
    fn test_reads_a_local_file_as_base64() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("secret.txt");
        fs::write(&path, "payload\n").unwrap();

        let module = SlurpModule;
        let output = module
            .execute(
                &params(&[("src", serde_json::json!(path.to_str().unwrap()))]),
                &ModuleContext::default(),
            )
            .unwrap();

        assert!(!output.changed, "reading a file changes nothing");
        let content = output.data.get("content").unwrap().as_str().unwrap();
        assert_eq!(STANDARD.decode(content).unwrap(), b"payload\n");
    }

    #[test]
    fn test_accepts_path_as_an_alias() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("secret.txt");
        fs::write(&path, "x").unwrap();

        let module = SlurpModule;
        let output = module
            .execute(
                &params(&[("path", serde_json::json!(path.to_str().unwrap()))]),
                &ModuleContext::default(),
            )
            .unwrap();
        assert_eq!(
            output.data.get("encoding").unwrap().as_str().unwrap(),
            "base64"
        );
    }

    #[test]
    fn test_missing_file_is_an_error() {
        let module = SlurpModule;
        let error = module
            .execute(
                &params(&[("src", serde_json::json!("/nonexistent/rustible-slurp"))]),
                &ModuleContext::default(),
            )
            .unwrap_err();
        assert!(error.to_string().contains("Failed to read"));
    }
}
