# Template Module Implementation for SSH over MiniJinja

This document describes the implementation of the template module that uses MiniJinja for rendering and SSH connections for uploading rendered content to remote hosts.

## Overview

The current `src/modules/template.rs` uses Tera and performs local file operations. The task is to modify it to:

1. **Use MiniJinja** instead of Tera (from `/home/artur/Repositories/rustible/src/template.rs`)
2. **Read template files locally** from the control node
3. **Render templates locally** using MiniJinja with variables from context
4. **Upload rendered content** to remote hosts via SSH using `connection.upload_content()`
5. **Support file attributes**: mode, owner, group parameters

## Key Implementation Changes

### 1. Update Imports

Replace Tera imports with MiniJinja integration:

```rust
use crate::connection::TransferOptions;
use crate::template::TemplateEngine;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
```

Remove these imports:
```rust
use std::io::Read;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use tera::{Context as TeraContext, Tera};
```

### 2. Replace Template Rendering Method

**Old (Tera):**
```rust
fn build_tera_context(context: &ModuleContext, extra_vars: Option<&serde_json::Value>) -> TeraContext {
    // ...
}

fn render_template(template_content: &str, tera_ctx: &TeraContext) -> ModuleResult<String> {
    let mut tera = Tera::default();
    // ... register filters ...
    tera.render("template", tera_ctx)
}
```

**New (MiniJinja):**
```rust
fn build_template_vars(
    context: &ModuleContext,
    extra_vars: Option<&serde_json::Value>,
) -> HashMap<String, serde_json::Value> {
    let mut vars = HashMap::new();

    // Add module variables
    for (key, value) in &context.vars {
        vars.insert(key.clone(), value.clone());
    }

    // Add facts
    vars.insert("ansible_facts".to_string(), serde_json::json!(context.facts));
    for (key, value) in &context.facts {
        vars.insert(key.clone(), value.clone());
    }

    // Add extra variables if provided
    if let Some(serde_json::Value::Object(extra)) = extra_vars {
        for (key, value) in extra {
            vars.insert(key.clone(), value.clone());
        }
    }

    vars
}

fn render_template(
    template_content: &str,
    vars: &HashMap<String, serde_json::Value>,
) -> ModuleResult<String> {
    let engine = TemplateEngine::new();
    engine
        .render(template_content, vars)
        .map_err(|e| ModuleError::TemplateError(format!("Failed to render template: {}", e)))
}
```

### 3. Update Module Trait Implementation

**Parameters:**
- Add support for `content` parameter (inline templates)
- Keep `src` parameter for file-based templates
- Add `owner` and `group` parameters
- Update `required_params()` to return `&["dest"]` only

**Validation:**
```rust
fn validate_params(&self, params: &ModuleParams) -> ModuleResult<()> {
    // Must have either src or content
    if params.get("src").is_none() && params.get("content").is_none() {
        return Err(ModuleError::MissingParameter(
            "Either 'src' or 'content' must be provided".to_string(),
        ));
    }
    // Must have dest
    if params.get("dest").is_none() {
        return Err(ModuleError::MissingParameter("dest".to_string()));
    }
    Ok(())
}
```

### 4. Rewrite Execute Method for SSH

**Key changes:**
1. Get connection from context (required)
2. Read template file locally (on control node)
3. Render template locally with MiniJinja
4. Check remote file existence using `connection.path_exists()`
5. Download remote file content using `connection.download_content()` for comparison
6. Upload rendered content using `connection.upload_content()` with TransferOptions

**Execute method structure:**
```rust
fn execute(&self, params: &ModuleParams, context: &ModuleContext) -> ModuleResult<ModuleOutput> {
    let dest = params.get_string_required("dest")?;
    let src = params.get_string("src")?;
    let content = params.get_string("content")?;
    let mode = params.get_u32("mode")?;
    let owner = params.get_string("owner")?;
    let group = params.get_string("group")?;
    let backup = params.get_bool_or("backup", false);
    let extra_vars = params.get("vars");

    // Get connection or fail if not available
    let connection = context
        .connection
        .as_ref()
        .ok_or_else(|| ModuleError::ExecutionFailed("No connection available".to_string()))?;

    // Get template content from src file or content parameter (read locally)
    let (template_content, src_display) = if let Some(ref content_str) = content {
        (content_str.clone(), "(inline content)".to_string())
    } else if let Some(ref src_str) = src {
        let src_path = Path::new(src_str);
        if !src_path.exists() {
            return Err(ModuleError::ExecutionFailed(format!(
                "Template source '{}' does not exist on control node",
                src_str
            )));
        }
        (fs::read_to_string(src_path)?, src_str.clone())
    } else {
        return Err(ModuleError::MissingParameter(
            "Either 'src' or 'content' must be provided".to_string(),
        ));
    };

    // Build variable context and render template locally
    let vars = Self::build_template_vars(context, extra_vars);
    let rendered = Self::render_template(&template_content, &vars)?;

    let dest_path = Path::new(&dest);

    // Check if remote dest needs updating
    let needs_update = if connection
        .path_exists(dest_path)
        .map_err(|e| ModuleError::ExecutionFailed(format!("Failed to check remote path: {}", e)))?
    {
        // Download current content and compare
        let current_content = connection
            .download_content(dest_path)
            .map_err(|e| ModuleError::ExecutionFailed(format!("Failed to download current file: {}", e)))?;
        let current_str = String::from_utf8_lossy(&current_content);
        current_str != rendered
    } else {
        true
    };

    // Handle check mode
    if context.check_mode {
        if !needs_update {
            return Ok(ModuleOutput::ok(format!("Template '{}' is already up to date", dest)));
        }

        let diff = if context.diff_mode {
            let before = if connection.path_exists(dest_path).unwrap_or(false) {
                let content = connection.download_content(dest_path).ok().unwrap_or_default();
                String::from_utf8_lossy(&content).to_string()
            } else {
                String::new()
            };
            Some(Diff::new(before, rendered.clone()))
        } else {
            None
        };

        let mut output = ModuleOutput::changed(format!(
            "Would render template '{}' to '{}'",
            src_display, dest
        ));

        if let Some(d) = diff {
            output = output.with_diff(d);
        }

        return Ok(output);
    }

    if !needs_update {
        return Ok(ModuleOutput::ok(format!("Template '{}' is already up to date", dest)));
    }

    // Create backup if requested and file exists
    if backup && connection.path_exists(dest_path).unwrap_or(false) {
        let backup_path = format!("{}~", dest);
        let current = connection
            .download_content(dest_path)
            .map_err(|e| ModuleError::ExecutionFailed(format!("Failed to download for backup: {}", e)))?;
        connection
            .upload_content(&current, Path::new(&backup_path), None)
            .map_err(|e| ModuleError::ExecutionFailed(format!("Failed to create backup: {}", e)))?;
    }

    // Build transfer options
    let mut transfer_opts = TransferOptions::new().with_create_dirs();
    if let Some(m) = mode {
        transfer_opts = transfer_opts.with_mode(m);
    }
    if let Some(o) = owner {
        transfer_opts = transfer_opts.with_owner(o);
    }
    if let Some(g) = group {
        transfer_opts = transfer_opts.with_group(g);
    }

    // Upload rendered content to remote destination
    connection
        .upload_content(rendered.as_bytes(), dest_path, Some(transfer_opts))
        .map_err(|e| {
            ModuleError::ExecutionFailed(format!("Failed to upload rendered template: {}", e))
        })?;

    let mut output = ModuleOutput::changed(format!(
        "Rendered template '{}' to '{}'",
        src_display, dest
    ));

    // Add metadata to output
    output = output
        .with_data("dest", serde_json::json!(dest))
        .with_data("src", serde_json::json!(src_display))
        .with_data("size", serde_json::json!(rendered.len()));

    if let Some(m) = mode {
        output = output.with_data("mode", serde_json::json!(format!("{:o}", m)));
    }

    Ok(output)
}
```

### 5. Update Diff Method

The diff method also needs to work with remote connections:

```rust
fn diff(&self, params: &ModuleParams, context: &ModuleContext) -> ModuleResult<Option<Diff>> {
    let dest = params.get_string_required("dest")?;
    let src = params.get_string("src")?;
    let content = params.get_string("content")?;
    let extra_vars = params.get("vars");

    // Get connection
    let connection = context
        .connection
        .as_ref()
        .ok_or_else(|| ModuleError::ExecutionFailed("No connection available".to_string()))?;

    // Get template content from src file or content parameter (read locally)
    let template_content = if let Some(ref content_str) = content {
        content_str.clone()
    } else if let Some(ref src_str) = src {
        let src_path = Path::new(src_str);
        if !src_path.exists() {
            return Err(ModuleError::ExecutionFailed(format!(
                "Template source '{}' does not exist on control node",
                src_str
            )));
        }
        fs::read_to_string(src_path)?
    } else {
        return Err(ModuleError::MissingParameter(
            "Either 'src' or 'content' must be provided".to_string(),
        ));
    };

    // Render template
    let vars = Self::build_template_vars(context, extra_vars);
    let rendered = Self::render_template(&template_content, &vars)?;

    // Get current content from remote host
    let dest_path = Path::new(&dest);
    let before = if connection
        .path_exists(dest_path)
        .map_err(|e| ModuleError::ExecutionFailed(format!("Failed to check remote path: {}", e)))?
    {
        let content = connection
            .download_content(dest_path)
            .map_err(|e| ModuleError::ExecutionFailed(format!("Failed to download file: {}", e)))?;
        String::from_utf8_lossy(&content).to_string()
    } else {
        String::new()
    };

    Ok(Some(Diff::new(before, rendered)))
}
```

### 6. Update Tests

Replace integration tests with unit tests that don't require SSH connections:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tempfile::TempDir;

    #[test]
    fn test_template_rendering() {
        let mut vars = HashMap::new();
        vars.insert("name".to_string(), serde_json::json!("World"));
        let rendered = TemplateModule::render_template("Hello, {{ name }}!", &vars).unwrap();
        assert_eq!(rendered, "Hello, World!");
    }

    #[test]
    fn test_template_with_loops() {
        let mut vars = HashMap::new();
        vars.insert("items".to_string(), serde_json::json!(["one", "two", "three"]));
        let rendered = TemplateModule::render_template(
            "{% for item in items %}{{ item }}\n{% endfor %}",
            &vars,
        ).unwrap();
        assert_eq!(rendered, "one\ntwo\nthree\n");
    }

    #[test]
    fn test_build_template_vars() {
        let mut vars = HashMap::new();
        vars.insert("var1".to_string(), serde_json::json!("value1"));
        let mut facts = HashMap::new();
        facts.insert("os_family".to_string(), serde_json::json!("Debian"));
        let context = ModuleContext::default().with_vars(vars).with_facts(facts);
        let template_vars = TemplateModule::build_template_vars(&context, None);

        assert_eq!(template_vars.get("var1"), Some(&serde_json::json!("value1")));
        assert_eq!(template_vars.get("os_family"), Some(&serde_json::json!("Debian")));
        assert!(template_vars.contains_key("ansible_facts"));
    }

    #[test]
    fn test_validate_params() {
        let module = TemplateModule;
        let mut params: ModuleParams = HashMap::new();
        params.insert("dest".to_string(), serde_json::json!("/tmp/file"));
        assert!(module.validate_params(&params).is_err());

        params.insert("src".to_string(), serde_json::json!("/tmp/template.j2"));
        assert!(module.validate_params(&params).is_ok());
    }
}
```

## Connection Trait Methods Used

From `/home/artur/Repositories/rustible/src/connection/mod.rs`:

```rust
async fn path_exists(&self, path: &Path) -> ConnectionResult<bool>;
async fn download_content(&self, remote_path: &Path) -> ConnectionResult<Vec<u8>>;
async fn upload_content(
    &self,
    content: &[u8],
    remote_path: &Path,
    options: Option<TransferOptions>,
) -> ConnectionResult<()>;
```

## TransferOptions Structure

```rust
#[derive(Debug, Clone, Default)]
pub struct TransferOptions {
    pub mode: Option<u32>,
    pub owner: Option<String>,
    pub group: Option<String>,
    pub create_dirs: bool,
    pub backup: bool,
}

impl TransferOptions {
    pub fn new() -> Self;
    pub fn with_mode(mut self, mode: u32) -> Self;
    pub fn with_owner(mut self, owner: impl Into<String>) -> Self;
    pub fn with_group(mut self, group: impl Into<String>) -> Self;
    pub fn with_create_dirs(mut self) -> Self;
}
```

## Benefits of This Implementation

1. **Uses MiniJinja**: Comprehensive Jinja2 compatibility with all Ansible filters from `/home/artur/Repositories/rustible/src/template.rs`
2. **SSH-aware**: Works over SSH connections to remote hosts
3. **Idempotent**: Checks remote file content before uploading
4. **Supports file attributes**: mode, owner, group via TransferOptions
5. **Backup support**: Creates backup on remote host before overwriting
6. **Check mode and diff**: Proper support for dry-run and showing changes
7. **Flexible input**: Supports both file-based (`src`) and inline (`content`) templates

## Module Classification

The module is correctly classified as `ModuleClassification::NativeTransport` because it:
- Uses native Rust SSH/SFTP operations
- Does not require remote Python execution
- Performs file transfer directly via the connection layer

This makes it suitable for parallel execution and provides better performance than Python-based modules.
