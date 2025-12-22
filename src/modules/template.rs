//! Template module - Render templates with Tera
//!
//! This module renders Tera templates (similar to Jinja2) and copies the result
//! to a destination file.

use super::{
    Diff, Module, ModuleContext, ModuleError, ModuleOutput, ModuleParams, ModuleResult, ParamExt,
};
use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;
use tera::{Context as TeraContext, Tera};

/// Module for rendering templates
pub struct TemplateModule;

impl TemplateModule {
    fn build_tera_context(
        context: &ModuleContext,
        extra_vars: Option<&serde_json::Value>,
    ) -> TeraContext {
        let mut tera_ctx = TeraContext::new();

        // Add variables
        for (key, value) in &context.vars {
            tera_ctx.insert(key, value);
        }

        // Add facts
        tera_ctx.insert("ansible_facts", &context.facts);
        for (key, value) in &context.facts {
            tera_ctx.insert(key, value);
        }

        // Add extra variables if provided
        if let Some(serde_json::Value::Object(vars)) = extra_vars {
            for (key, value) in vars {
                tera_ctx.insert(key, value);
            }
        }

        tera_ctx
    }

    fn render_template(template_content: &str, tera_ctx: &TeraContext) -> ModuleResult<String> {
        let mut tera = Tera::default();

        // Add custom filters similar to Ansible/Jinja2
        tera.register_filter(
            "default",
            |value: &tera::Value, args: &HashMap<String, tera::Value>| {
                if value.is_null() || (value.is_string() && value.as_str().unwrap().is_empty()) {
                    if let Some(default) = args.get("value") {
                        return Ok(default.clone());
                    }
                }
                Ok(value.clone())
            },
        );

        tera.register_filter(
            "upper",
            |value: &tera::Value, _args: &HashMap<String, tera::Value>| match value {
                tera::Value::String(s) => Ok(tera::Value::String(s.to_uppercase())),
                _ => Ok(value.clone()),
            },
        );

        tera.register_filter(
            "lower",
            |value: &tera::Value, _args: &HashMap<String, tera::Value>| match value {
                tera::Value::String(s) => Ok(tera::Value::String(s.to_lowercase())),
                _ => Ok(value.clone()),
            },
        );

        tera.register_filter(
            "trim",
            |value: &tera::Value, _args: &HashMap<String, tera::Value>| match value {
                tera::Value::String(s) => Ok(tera::Value::String(s.trim().to_string())),
                _ => Ok(value.clone()),
            },
        );

        tera.register_filter(
            "replace",
            |value: &tera::Value, args: &HashMap<String, tera::Value>| match value {
                tera::Value::String(s) => {
                    let from = args.get("from").and_then(|v| v.as_str()).unwrap_or("");
                    let to = args.get("to").and_then(|v| v.as_str()).unwrap_or("");
                    Ok(tera::Value::String(s.replace(from, to)))
                }
                _ => Ok(value.clone()),
            },
        );

        tera.register_filter(
            "join",
            |value: &tera::Value, args: &HashMap<String, tera::Value>| match value {
                tera::Value::Array(arr) => {
                    let sep = args.get("sep").and_then(|v| v.as_str()).unwrap_or(",");
                    let joined: Vec<String> = arr
                        .iter()
                        .map(|v| match v {
                            tera::Value::String(s) => s.clone(),
                            _ => v.to_string(),
                        })
                        .collect();
                    Ok(tera::Value::String(joined.join(sep)))
                }
                _ => Ok(value.clone()),
            },
        );

        tera.add_raw_template("template", template_content)
            .map_err(|e| ModuleError::TemplateError(format!("Failed to parse template: {}", e)))?;

        tera.render("template", tera_ctx)
            .map_err(|e| ModuleError::TemplateError(format!("Failed to render template: {}", e)))
    }

    fn get_file_checksum(path: &Path) -> std::io::Result<String> {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut file = fs::File::open(path)?;
        let mut contents = Vec::new();
        file.read_to_end(&mut contents)?;

        let mut hasher = DefaultHasher::new();
        contents.hash(&mut hasher);
        Ok(format!("{:x}", hasher.finish()))
    }

    fn create_backup(dest: &Path, backup_suffix: &str) -> ModuleResult<Option<String>> {
        if dest.exists() {
            let backup_path = format!("{}{}", dest.display(), backup_suffix);
            fs::copy(dest, &backup_path)?;
            Ok(Some(backup_path))
        } else {
            Ok(None)
        }
    }

    fn set_permissions(path: &Path, mode: Option<u32>) -> ModuleResult<bool> {
        if let Some(mode) = mode {
            let current = fs::metadata(path)?.permissions().mode() & 0o7777;
            if current != mode {
                fs::set_permissions(path, fs::Permissions::from_mode(mode))?;
                return Ok(true);
            }
        }
        Ok(false)
    }
}

impl Module for TemplateModule {
    fn name(&self) -> &'static str {
        "template"
    }

    fn description(&self) -> &'static str {
        "Render Tera/Jinja2 templates to a destination"
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
        let src_path = Path::new(&src);
        let dest_path = Path::new(&dest);
        let backup = params.get_bool_or("backup", false);
        let backup_suffix = params
            .get_string("backup_suffix")?
            .unwrap_or_else(|| "~".to_string());
        let mode = params.get_u32("mode")?;
        let extra_vars = params.get("vars");

        // Check source exists
        if !src_path.exists() {
            return Err(ModuleError::ExecutionFailed(format!(
                "Template source '{}' does not exist",
                src
            )));
        }

        // Read template content
        let template_content = fs::read_to_string(src_path).map_err(|e| ModuleError::Io(e))?;

        // Build context and render
        let tera_ctx = Self::build_tera_context(context, extra_vars);
        let rendered = Self::render_template(&template_content, &tera_ctx)?;

        // Check if dest needs updating
        let needs_update = if dest_path.exists() {
            let current_content = fs::read_to_string(dest_path)?;
            current_content != rendered
        } else {
            true
        };

        if !needs_update {
            // Check if only permissions need updating
            let perm_changed = if let Some(m) = mode {
                if dest_path.exists() {
                    let current = fs::metadata(dest_path)?.permissions().mode() & 0o7777;
                    current != m
                } else {
                    false
                }
            } else {
                false
            };

            if perm_changed {
                if context.check_mode {
                    return Ok(ModuleOutput::changed(format!(
                        "Would change permissions on '{}'",
                        dest
                    )));
                }
                Self::set_permissions(dest_path, mode)?;
                return Ok(ModuleOutput::changed(format!(
                    "Changed permissions on '{}'",
                    dest
                )));
            }

            return Ok(ModuleOutput::ok(format!(
                "Template '{}' is already up to date",
                dest
            )));
        }

        // In check mode, return what would happen
        if context.check_mode {
            let diff = if context.diff_mode {
                let before = if dest_path.exists() {
                    fs::read_to_string(dest_path).unwrap_or_default()
                } else {
                    String::new()
                };
                Some(Diff::new(before, rendered.clone()))
            } else {
                None
            };

            let mut output =
                ModuleOutput::changed(format!("Would render template '{}' to '{}'", src, dest));

            if let Some(d) = diff {
                output = output.with_diff(d);
            }

            return Ok(output);
        }

        // Create backup if requested
        let backup_file = if backup {
            Self::create_backup(dest_path, &backup_suffix)?
        } else {
            None
        };

        // Create parent directories if needed
        if let Some(parent) = dest_path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)?;
            }
        }

        // Write rendered content
        fs::write(dest_path, &rendered)?;

        // Set permissions
        let perm_changed = Self::set_permissions(dest_path, mode)?;

        let mut output =
            ModuleOutput::changed(format!("Rendered template '{}' to '{}'", src, dest));

        if let Some(backup_path) = backup_file {
            output = output.with_data("backup_file", serde_json::json!(backup_path));
        }

        if perm_changed {
            output = output.with_data("mode_changed", serde_json::json!(true));
        }

        // Add file info to output
        let meta = fs::metadata(dest_path)?;
        output = output
            .with_data("dest", serde_json::json!(dest))
            .with_data("src", serde_json::json!(src))
            .with_data("size", serde_json::json!(meta.len()))
            .with_data(
                "mode",
                serde_json::json!(format!("{:o}", meta.permissions().mode() & 0o7777)),
            )
            .with_data("uid", serde_json::json!(meta.uid()))
            .with_data("gid", serde_json::json!(meta.gid()));

        Ok(output)
    }

    fn check(&self, params: &ModuleParams, context: &ModuleContext) -> ModuleResult<ModuleOutput> {
        let check_context = ModuleContext {
            check_mode: true,
            ..context.clone()
        };
        self.execute(params, &check_context)
    }

    fn diff(&self, params: &ModuleParams, context: &ModuleContext) -> ModuleResult<Option<Diff>> {
        let src = params.get_string_required("src")?;
        let dest = params.get_string_required("dest")?;
        let src_path = Path::new(&src);
        let dest_path = Path::new(&dest);
        let extra_vars = params.get("vars");

        if !src_path.exists() {
            return Err(ModuleError::ExecutionFailed(format!(
                "Template source '{}' does not exist",
                src
            )));
        }

        let template_content = fs::read_to_string(src_path)?;
        let tera_ctx = Self::build_tera_context(context, extra_vars);
        let rendered = Self::render_template(&template_content, &tera_ctx)?;

        let before = if dest_path.exists() {
            fs::read_to_string(dest_path).unwrap_or_default()
        } else {
            String::new()
        };

        Ok(Some(Diff::new(before, rendered)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tempfile::TempDir;

    #[test]
    fn test_template_basic() {
        let temp = TempDir::new().unwrap();
        let src = temp.path().join("template.txt.j2");
        let dest = temp.path().join("output.txt");

        fs::write(&src, "Hello, {{ name }}!").unwrap();

        let module = TemplateModule;
        let mut params: ModuleParams = HashMap::new();
        params.insert("src".to_string(), serde_json::json!(src.to_str().unwrap()));
        params.insert(
            "dest".to_string(),
            serde_json::json!(dest.to_str().unwrap()),
        );

        let mut vars = HashMap::new();
        vars.insert("name".to_string(), serde_json::json!("World"));

        let context = ModuleContext::default().with_vars(vars);
        let result = module.execute(&params, &context).unwrap();

        assert!(result.changed);
        assert!(dest.exists());
        assert_eq!(fs::read_to_string(&dest).unwrap(), "Hello, World!");
    }

    #[test]
    fn test_template_with_loops() {
        let temp = TempDir::new().unwrap();
        let src = temp.path().join("template.txt.j2");
        let dest = temp.path().join("output.txt");

        fs::write(&src, "{% for item in items %}{{ item }}\n{% endfor %}").unwrap();

        let module = TemplateModule;
        let mut params: ModuleParams = HashMap::new();
        params.insert("src".to_string(), serde_json::json!(src.to_str().unwrap()));
        params.insert(
            "dest".to_string(),
            serde_json::json!(dest.to_str().unwrap()),
        );

        let mut vars = HashMap::new();
        vars.insert(
            "items".to_string(),
            serde_json::json!(["one", "two", "three"]),
        );

        let context = ModuleContext::default().with_vars(vars);
        let result = module.execute(&params, &context).unwrap();

        assert!(result.changed);
        assert_eq!(fs::read_to_string(&dest).unwrap(), "one\ntwo\nthree\n");
    }

    #[test]
    fn test_template_with_conditionals() {
        let temp = TempDir::new().unwrap();
        let src = temp.path().join("template.txt.j2");
        let dest = temp.path().join("output.txt");

        fs::write(
            &src,
            "{% if enabled %}Feature enabled{% else %}Feature disabled{% endif %}",
        )
        .unwrap();

        let module = TemplateModule;
        let mut params: ModuleParams = HashMap::new();
        params.insert("src".to_string(), serde_json::json!(src.to_str().unwrap()));
        params.insert(
            "dest".to_string(),
            serde_json::json!(dest.to_str().unwrap()),
        );

        let mut vars = HashMap::new();
        vars.insert("enabled".to_string(), serde_json::json!(true));

        let context = ModuleContext::default().with_vars(vars);
        let result = module.execute(&params, &context).unwrap();

        assert!(result.changed);
        assert_eq!(fs::read_to_string(&dest).unwrap(), "Feature enabled");
    }

    #[test]
    fn test_template_idempotent() {
        let temp = TempDir::new().unwrap();
        let src = temp.path().join("template.txt.j2");
        let dest = temp.path().join("output.txt");

        fs::write(&src, "Hello, {{ name }}!").unwrap();
        fs::write(&dest, "Hello, World!").unwrap();

        let module = TemplateModule;
        let mut params: ModuleParams = HashMap::new();
        params.insert("src".to_string(), serde_json::json!(src.to_str().unwrap()));
        params.insert(
            "dest".to_string(),
            serde_json::json!(dest.to_str().unwrap()),
        );

        let mut vars = HashMap::new();
        vars.insert("name".to_string(), serde_json::json!("World"));

        let context = ModuleContext::default().with_vars(vars);
        let result = module.execute(&params, &context).unwrap();

        assert!(!result.changed);
    }

    #[test]
    fn test_template_check_mode() {
        let temp = TempDir::new().unwrap();
        let src = temp.path().join("template.txt.j2");
        let dest = temp.path().join("output.txt");

        fs::write(&src, "Hello, {{ name }}!").unwrap();

        let module = TemplateModule;
        let mut params: ModuleParams = HashMap::new();
        params.insert("src".to_string(), serde_json::json!(src.to_str().unwrap()));
        params.insert(
            "dest".to_string(),
            serde_json::json!(dest.to_str().unwrap()),
        );

        let mut vars = HashMap::new();
        vars.insert("name".to_string(), serde_json::json!("World"));

        let context = ModuleContext::default()
            .with_vars(vars)
            .with_check_mode(true);
        let result = module.check(&params, &context).unwrap();

        assert!(result.changed);
        assert!(result.msg.contains("Would render"));
        assert!(!dest.exists()); // File should not be created in check mode
    }

    #[test]
    fn test_template_filters() {
        let temp = TempDir::new().unwrap();
        let src = temp.path().join("template.txt.j2");
        let dest = temp.path().join("output.txt");

        fs::write(&src, "{{ name | upper }}").unwrap();

        let module = TemplateModule;
        let mut params: ModuleParams = HashMap::new();
        params.insert("src".to_string(), serde_json::json!(src.to_str().unwrap()));
        params.insert(
            "dest".to_string(),
            serde_json::json!(dest.to_str().unwrap()),
        );

        let mut vars = HashMap::new();
        vars.insert("name".to_string(), serde_json::json!("hello"));

        let context = ModuleContext::default().with_vars(vars);
        let result = module.execute(&params, &context).unwrap();

        assert!(result.changed);
        assert_eq!(fs::read_to_string(&dest).unwrap(), "HELLO");
    }
}
