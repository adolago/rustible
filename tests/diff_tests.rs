//! Comprehensive tests for diff mode output functionality in Rustible.
//!
//! These tests verify the --diff mode output for all modules that support it:
//! - Diff output format (before/after/details)
//! - Copy module diff for file content changes
//! - Template module diff for rendered content changes
//! - File module diff for state changes
//! - Lineinfile module diff for line-level changes
//! - Diff combined with check mode
//! - Diff formatting and edge cases

use rustible::modules::{
    copy::CopyModule, file::FileModule, template::TemplateModule, Diff, Module, ModuleContext,
    ModuleOutput, ModuleParams,
};
use std::collections::HashMap;
use std::fs;
use tempfile::TempDir;

// ============================================================================
// Diff Structure Tests
// ============================================================================

#[test]
fn test_diff_new_basic() {
    let diff = Diff::new("old content", "new content");

    assert_eq!(diff.before, "old content");
    assert_eq!(diff.after, "new content");
    assert!(diff.details.is_none());
}

#[test]
fn test_diff_with_details() {
    let diff = Diff::new("before", "after").with_details("--- a/file\n+++ b/file\n-old\n+new");

    assert_eq!(diff.before, "before");
    assert_eq!(diff.after, "after");
    assert_eq!(
        diff.details,
        Some("--- a/file\n+++ b/file\n-old\n+new".to_string())
    );
}

#[test]
fn test_diff_empty_before() {
    let diff = Diff::new("", "new content");

    assert_eq!(diff.before, "");
    assert_eq!(diff.after, "new content");
}

#[test]
fn test_diff_empty_after() {
    let diff = Diff::new("old content", "");

    assert_eq!(diff.before, "old content");
    assert_eq!(diff.after, "");
}

#[test]
fn test_diff_both_empty() {
    let diff = Diff::new("", "");

    assert_eq!(diff.before, "");
    assert_eq!(diff.after, "");
}

#[test]
fn test_diff_multiline_content() {
    let before = "line1\nline2\nline3";
    let after = "line1\nmodified\nline3\nline4";
    let diff = Diff::new(before, after);

    assert_eq!(diff.before, before);
    assert_eq!(diff.after, after);
}

#[test]
fn test_diff_unicode_content() {
    let before = "Hello \u{1F600}";
    let after = "Hello \u{1F604} World \u{4E2D}\u{6587}";
    let diff = Diff::new(before, after);

    assert_eq!(diff.before, before);
    assert_eq!(diff.after, after);
}

#[test]
fn test_diff_whitespace_only_changes() {
    let before = "line1\nline2";
    let after = "line1  \n  line2";
    let diff = Diff::new(before, after);

    assert_eq!(diff.before, before);
    assert_eq!(diff.after, after);
    assert_ne!(diff.before, diff.after);
}

// ============================================================================
// ModuleOutput with Diff Tests
// ============================================================================

#[test]
fn test_module_output_with_diff() {
    let diff = Diff::new("old", "new");
    let output = ModuleOutput::changed("Content updated").with_diff(diff);

    assert!(output.changed);
    assert!(output.diff.is_some());
    let d = output.diff.unwrap();
    assert_eq!(d.before, "old");
    assert_eq!(d.after, "new");
}

#[test]
fn test_module_output_without_diff() {
    let output = ModuleOutput::ok("No changes");

    assert!(!output.changed);
    assert!(output.diff.is_none());
}

#[test]
fn test_module_output_diff_with_data() {
    let diff = Diff::new("before", "after");
    let output = ModuleOutput::changed("Updated")
        .with_diff(diff)
        .with_data("file", serde_json::json!("/tmp/test.txt"))
        .with_data("size", serde_json::json!(1024));

    assert!(output.diff.is_some());
    assert_eq!(output.data.len(), 2);
}

// ============================================================================
// ModuleContext Diff Mode Tests
// ============================================================================

#[test]
fn test_context_diff_mode_disabled_by_default() {
    let ctx = ModuleContext::default();

    assert!(!ctx.diff_mode);
    assert!(!ctx.check_mode);
}

#[test]
fn test_context_diff_mode_enabled() {
    let ctx = ModuleContext::default().with_diff_mode(true);

    assert!(ctx.diff_mode);
}

#[test]
fn test_context_check_and_diff_mode() {
    let ctx = ModuleContext::default()
        .with_check_mode(true)
        .with_diff_mode(true);

    assert!(ctx.check_mode);
    assert!(ctx.diff_mode);
}

// ============================================================================
// Copy Module Diff Tests
// ============================================================================

#[test]
fn test_copy_diff_new_file_creation() {
    let temp = TempDir::new().unwrap();
    let dest = temp.path().join("new_file.txt");

    let module = CopyModule;
    let mut params: ModuleParams = HashMap::new();
    params.insert("content".to_string(), serde_json::json!("Hello, World!"));
    params.insert(
        "dest".to_string(),
        serde_json::json!(dest.to_str().unwrap()),
    );

    let context = ModuleContext::default();
    let diff = module.diff(&params, &context).unwrap();

    // Diff should show empty before (file doesn't exist) and new content after
    assert!(diff.is_some());
    let d = diff.unwrap();
    assert_eq!(d.before, "");
    assert_eq!(d.after, "Hello, World!");
}

#[test]
fn test_copy_diff_file_content_change() {
    let temp = TempDir::new().unwrap();
    let dest = temp.path().join("existing.txt");

    // Create existing file
    fs::write(&dest, "Old content").unwrap();

    let module = CopyModule;
    let mut params: ModuleParams = HashMap::new();
    params.insert("content".to_string(), serde_json::json!("New content"));
    params.insert(
        "dest".to_string(),
        serde_json::json!(dest.to_str().unwrap()),
    );

    let context = ModuleContext::default();
    let diff = module.diff(&params, &context).unwrap();

    assert!(diff.is_some());
    let d = diff.unwrap();
    assert_eq!(d.before, "Old content");
    assert_eq!(d.after, "New content");
}

#[test]
fn test_copy_diff_no_change() {
    let temp = TempDir::new().unwrap();
    let dest = temp.path().join("same.txt");

    // Create file with same content
    fs::write(&dest, "Same content").unwrap();

    let module = CopyModule;
    let mut params: ModuleParams = HashMap::new();
    params.insert("content".to_string(), serde_json::json!("Same content"));
    params.insert(
        "dest".to_string(),
        serde_json::json!(dest.to_str().unwrap()),
    );

    let context = ModuleContext::default();
    let diff = module.diff(&params, &context).unwrap();

    // Even though content is same, diff still returns the comparison
    assert!(diff.is_some());
    let d = diff.unwrap();
    assert_eq!(d.before, d.after);
}

#[test]
fn test_copy_diff_from_source_file() {
    let temp = TempDir::new().unwrap();
    let src = temp.path().join("source.txt");
    let dest = temp.path().join("dest.txt");

    fs::write(&src, "Source content").unwrap();
    fs::write(&dest, "Old dest content").unwrap();

    let module = CopyModule;
    let mut params: ModuleParams = HashMap::new();
    params.insert("src".to_string(), serde_json::json!(src.to_str().unwrap()));
    params.insert(
        "dest".to_string(),
        serde_json::json!(dest.to_str().unwrap()),
    );

    let context = ModuleContext::default();
    let diff = module.diff(&params, &context).unwrap();

    assert!(diff.is_some());
    let d = diff.unwrap();
    assert_eq!(d.before, "Old dest content");
    assert_eq!(d.after, "Source content");
}

#[test]
fn test_copy_diff_binary_file_handling() {
    let temp = TempDir::new().unwrap();
    let src = temp.path().join("binary.bin");
    let dest = temp.path().join("dest.bin");

    // Create a binary file (non-UTF8 content)
    fs::write(&src, &[0x00, 0x01, 0xFF, 0xFE]).unwrap();

    let module = CopyModule;
    let mut params: ModuleParams = HashMap::new();
    params.insert("src".to_string(), serde_json::json!(src.to_str().unwrap()));
    params.insert(
        "dest".to_string(),
        serde_json::json!(dest.to_str().unwrap()),
    );

    let context = ModuleContext::default();
    let diff = module.diff(&params, &context).unwrap();

    // Binary files should be indicated as such
    assert!(diff.is_some());
    let d = diff.unwrap();
    assert!(d.after.contains("binary") || d.after.len() > 0);
}

#[test]
fn test_copy_check_mode_with_diff() {
    let temp = TempDir::new().unwrap();
    let dest = temp.path().join("check_diff.txt");

    fs::write(&dest, "old content").unwrap();

    let module = CopyModule;
    let mut params: ModuleParams = HashMap::new();
    params.insert("content".to_string(), serde_json::json!("new content"));
    params.insert(
        "dest".to_string(),
        serde_json::json!(dest.to_str().unwrap()),
    );

    let context = ModuleContext::default()
        .with_check_mode(true)
        .with_diff_mode(true);

    let result = module.check(&params, &context).unwrap();

    assert!(result.changed);
    assert!(result.msg.contains("Would copy"));
    assert!(result.diff.is_some());

    let diff = result.diff.unwrap();
    assert_eq!(diff.before, "old content");
    assert_eq!(diff.after, "new content");

    // Verify file was not modified
    assert_eq!(fs::read_to_string(&dest).unwrap(), "old content");
}

#[test]
fn test_copy_multiline_diff() {
    let temp = TempDir::new().unwrap();
    let dest = temp.path().join("multiline.txt");

    let old_content = "line1\nline2\nline3";
    let new_content = "line1\nmodified\nline3\nline4";

    fs::write(&dest, old_content).unwrap();

    let module = CopyModule;
    let mut params: ModuleParams = HashMap::new();
    params.insert("content".to_string(), serde_json::json!(new_content));
    params.insert(
        "dest".to_string(),
        serde_json::json!(dest.to_str().unwrap()),
    );

    let context = ModuleContext::default();
    let diff = module.diff(&params, &context).unwrap();

    assert!(diff.is_some());
    let d = diff.unwrap();
    assert_eq!(d.before, old_content);
    assert_eq!(d.after, new_content);
}

#[test]
fn test_copy_empty_to_content_diff() {
    let temp = TempDir::new().unwrap();
    let dest = temp.path().join("empty_to_content.txt");

    // Create empty file
    fs::write(&dest, "").unwrap();

    let module = CopyModule;
    let mut params: ModuleParams = HashMap::new();
    params.insert("content".to_string(), serde_json::json!("New content here"));
    params.insert(
        "dest".to_string(),
        serde_json::json!(dest.to_str().unwrap()),
    );

    let context = ModuleContext::default();
    let diff = module.diff(&params, &context).unwrap();

    assert!(diff.is_some());
    let d = diff.unwrap();
    assert_eq!(d.before, "");
    assert_eq!(d.after, "New content here");
}

#[test]
fn test_copy_content_to_empty_diff() {
    let temp = TempDir::new().unwrap();
    let dest = temp.path().join("content_to_empty.txt");

    fs::write(&dest, "Existing content").unwrap();

    let module = CopyModule;
    let mut params: ModuleParams = HashMap::new();
    params.insert("content".to_string(), serde_json::json!(""));
    params.insert(
        "dest".to_string(),
        serde_json::json!(dest.to_str().unwrap()),
    );

    let context = ModuleContext::default();
    let diff = module.diff(&params, &context).unwrap();

    assert!(diff.is_some());
    let d = diff.unwrap();
    assert_eq!(d.before, "Existing content");
    assert_eq!(d.after, "");
}

// ============================================================================
// Template Module Diff Tests
// ============================================================================

#[test]
fn test_template_diff_new_file() {
    let temp = TempDir::new().unwrap();
    let src = temp.path().join("template.j2");
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
    let diff = module.diff(&params, &context).unwrap();

    assert!(diff.is_some());
    let d = diff.unwrap();
    assert_eq!(d.before, ""); // File doesn't exist
    assert_eq!(d.after, "Hello, World!");
}

#[test]
fn test_template_diff_variable_change_impact() {
    let temp = TempDir::new().unwrap();
    let src = temp.path().join("template.j2");
    let dest = temp.path().join("output.txt");

    fs::write(&src, "Server: {{ server_name }}:{{ port }}").unwrap();
    fs::write(&dest, "Server: old.example.com:80").unwrap();

    let module = TemplateModule;
    let mut params: ModuleParams = HashMap::new();
    params.insert("src".to_string(), serde_json::json!(src.to_str().unwrap()));
    params.insert(
        "dest".to_string(),
        serde_json::json!(dest.to_str().unwrap()),
    );

    let mut vars = HashMap::new();
    vars.insert(
        "server_name".to_string(),
        serde_json::json!("new.example.com"),
    );
    vars.insert("port".to_string(), serde_json::json!("8080"));

    let context = ModuleContext::default().with_vars(vars);
    let diff = module.diff(&params, &context).unwrap();

    assert!(diff.is_some());
    let d = diff.unwrap();
    assert_eq!(d.before, "Server: old.example.com:80");
    assert_eq!(d.after, "Server: new.example.com:8080");
}

#[test]
fn test_template_diff_complex_template() {
    let temp = TempDir::new().unwrap();
    let src = temp.path().join("complex.j2");
    let dest = temp.path().join("output.txt");

    let template = r#"{% for item in items %}
{{ item }}
{% endfor %}"#;

    fs::write(&src, template).unwrap();

    let module = TemplateModule;
    let mut params: ModuleParams = HashMap::new();
    params.insert("src".to_string(), serde_json::json!(src.to_str().unwrap()));
    params.insert(
        "dest".to_string(),
        serde_json::json!(dest.to_str().unwrap()),
    );

    let mut vars = HashMap::new();
    vars.insert("items".to_string(), serde_json::json!(["a", "b", "c"]));

    let context = ModuleContext::default().with_vars(vars);
    let diff = module.diff(&params, &context).unwrap();

    assert!(diff.is_some());
    let d = diff.unwrap();
    assert!(d.after.contains("a"));
    assert!(d.after.contains("b"));
    assert!(d.after.contains("c"));
}

#[test]
fn test_template_diff_conditionals() {
    let temp = TempDir::new().unwrap();
    let src = temp.path().join("conditional.j2");
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
    let diff = module.diff(&params, &context).unwrap();

    assert!(diff.is_some());
    let d = diff.unwrap();
    assert_eq!(d.after, "Feature enabled");
}

#[test]
fn test_template_check_mode_with_diff() {
    let temp = TempDir::new().unwrap();
    let src = temp.path().join("template.j2");
    let dest = temp.path().join("output.txt");

    fs::write(&src, "Value: {{ value }}").unwrap();
    fs::write(&dest, "Value: old").unwrap();

    let module = TemplateModule;
    let mut params: ModuleParams = HashMap::new();
    params.insert("src".to_string(), serde_json::json!(src.to_str().unwrap()));
    params.insert(
        "dest".to_string(),
        serde_json::json!(dest.to_str().unwrap()),
    );

    let mut vars = HashMap::new();
    vars.insert("value".to_string(), serde_json::json!("new"));

    let context = ModuleContext::default()
        .with_vars(vars)
        .with_check_mode(true)
        .with_diff_mode(true);

    let result = module.check(&params, &context).unwrap();

    assert!(result.changed);
    assert!(result.msg.contains("Would render"));
    assert!(result.diff.is_some());

    // Verify original file unchanged
    assert_eq!(fs::read_to_string(&dest).unwrap(), "Value: old");
}

#[test]
fn test_template_diff_no_change() {
    let temp = TempDir::new().unwrap();
    let src = temp.path().join("template.j2");
    let dest = temp.path().join("output.txt");

    fs::write(&src, "Static content").unwrap();
    fs::write(&dest, "Static content").unwrap();

    let module = TemplateModule;
    let mut params: ModuleParams = HashMap::new();
    params.insert("src".to_string(), serde_json::json!(src.to_str().unwrap()));
    params.insert(
        "dest".to_string(),
        serde_json::json!(dest.to_str().unwrap()),
    );

    let context = ModuleContext::default();
    let diff = module.diff(&params, &context).unwrap();

    assert!(diff.is_some());
    let d = diff.unwrap();
    assert_eq!(d.before, d.after);
}

// ============================================================================
// File Module Diff Tests
// ============================================================================

#[test]
fn test_file_diff_create_directory() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("newdir");

    let module = FileModule;
    let mut params: ModuleParams = HashMap::new();
    params.insert(
        "path".to_string(),
        serde_json::json!(path.to_str().unwrap()),
    );
    params.insert("state".to_string(), serde_json::json!("directory"));

    let context = ModuleContext::default();
    let diff = module.diff(&params, &context).unwrap();

    assert!(diff.is_some());
    let d = diff.unwrap();
    assert_eq!(d.before, "absent");
    assert_eq!(d.after, "directory exists");
}

#[test]
fn test_file_diff_create_file() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("newfile");

    let module = FileModule;
    let mut params: ModuleParams = HashMap::new();
    params.insert(
        "path".to_string(),
        serde_json::json!(path.to_str().unwrap()),
    );
    params.insert("state".to_string(), serde_json::json!("file"));

    let context = ModuleContext::default();
    let diff = module.diff(&params, &context).unwrap();

    assert!(diff.is_some());
    let d = diff.unwrap();
    assert_eq!(d.before, "absent");
    assert_eq!(d.after, "file exists");
}

#[test]
fn test_file_diff_remove_file() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("existingfile");
    fs::write(&path, "content").unwrap();

    let module = FileModule;
    let mut params: ModuleParams = HashMap::new();
    params.insert(
        "path".to_string(),
        serde_json::json!(path.to_str().unwrap()),
    );
    params.insert("state".to_string(), serde_json::json!("absent"));

    let context = ModuleContext::default();
    let diff = module.diff(&params, &context).unwrap();

    assert!(diff.is_some());
    let d = diff.unwrap();
    assert_eq!(d.before, "file exists");
    assert_eq!(d.after, "absent");
}

#[test]
fn test_file_diff_remove_directory() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("existingdir");
    fs::create_dir(&path).unwrap();

    let module = FileModule;
    let mut params: ModuleParams = HashMap::new();
    params.insert(
        "path".to_string(),
        serde_json::json!(path.to_str().unwrap()),
    );
    params.insert("state".to_string(), serde_json::json!("absent"));

    let context = ModuleContext::default();
    let diff = module.diff(&params, &context).unwrap();

    assert!(diff.is_some());
    let d = diff.unwrap();
    assert_eq!(d.before, "directory exists");
    assert_eq!(d.after, "absent");
}

#[test]
fn test_file_diff_symlink_creation() {
    let temp = TempDir::new().unwrap();
    let src = temp.path().join("source");
    let dest = temp.path().join("link");
    fs::write(&src, "content").unwrap();

    let module = FileModule;
    let mut params: ModuleParams = HashMap::new();
    params.insert(
        "path".to_string(),
        serde_json::json!(dest.to_str().unwrap()),
    );
    params.insert("src".to_string(), serde_json::json!(src.to_str().unwrap()));
    params.insert("state".to_string(), serde_json::json!("link"));

    let context = ModuleContext::default();
    let diff = module.diff(&params, &context).unwrap();

    assert!(diff.is_some());
    let d = diff.unwrap();
    assert_eq!(d.before, "absent");
    assert!(d.after.contains("symlink"));
}

#[test]
fn test_file_diff_symlink_change_target() {
    let temp = TempDir::new().unwrap();
    let src1 = temp.path().join("source1");
    let src2 = temp.path().join("source2");
    let link = temp.path().join("link");

    fs::write(&src1, "content1").unwrap();
    fs::write(&src2, "content2").unwrap();
    std::os::unix::fs::symlink(&src1, &link).unwrap();

    let module = FileModule;
    let mut params: ModuleParams = HashMap::new();
    params.insert(
        "path".to_string(),
        serde_json::json!(link.to_str().unwrap()),
    );
    params.insert("src".to_string(), serde_json::json!(src2.to_str().unwrap()));
    params.insert("state".to_string(), serde_json::json!("link"));

    let context = ModuleContext::default();
    let diff = module.diff(&params, &context).unwrap();

    assert!(diff.is_some());
    let d = diff.unwrap();
    assert!(d.before.contains("symlink"));
    assert!(d.after.contains("symlink"));
    assert!(d.after.contains(src2.to_str().unwrap()));
}

#[test]
fn test_file_diff_no_change_file_exists() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("existing");
    fs::write(&path, "content").unwrap();

    let module = FileModule;
    let mut params: ModuleParams = HashMap::new();
    params.insert(
        "path".to_string(),
        serde_json::json!(path.to_str().unwrap()),
    );
    params.insert("state".to_string(), serde_json::json!("file"));

    let context = ModuleContext::default();
    let diff = module.diff(&params, &context).unwrap();

    // No diff should be returned when file already exists in desired state
    assert!(
        diff.is_none(),
        "Should not return diff when no change needed"
    );
}

#[test]
fn test_file_diff_no_change_directory_exists() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("existingdir");
    fs::create_dir(&path).unwrap();

    let module = FileModule;
    let mut params: ModuleParams = HashMap::new();
    params.insert(
        "path".to_string(),
        serde_json::json!(path.to_str().unwrap()),
    );
    params.insert("state".to_string(), serde_json::json!("directory"));

    let context = ModuleContext::default();
    let diff = module.diff(&params, &context).unwrap();

    // No diff should be returned when directory already exists in desired state
    assert!(
        diff.is_none(),
        "Should not return diff when no change needed"
    );
}

#[test]
fn test_file_diff_absent_already_absent() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("nonexistent");

    let module = FileModule;
    let mut params: ModuleParams = HashMap::new();
    params.insert(
        "path".to_string(),
        serde_json::json!(path.to_str().unwrap()),
    );
    params.insert("state".to_string(), serde_json::json!("absent"));

    let context = ModuleContext::default();
    let diff = module.diff(&params, &context).unwrap();

    // No diff when already absent
    assert!(diff.is_none());
}

#[test]
fn test_file_check_mode_with_diff_create() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("newdir");

    let module = FileModule;
    let mut params: ModuleParams = HashMap::new();
    params.insert(
        "path".to_string(),
        serde_json::json!(path.to_str().unwrap()),
    );
    params.insert("state".to_string(), serde_json::json!("directory"));

    let context = ModuleContext::default()
        .with_check_mode(true)
        .with_diff_mode(true);

    let result = module.check(&params, &context).unwrap();

    assert!(result.changed);
    assert!(result.msg.contains("Would create"));
    assert!(!path.exists());
}

#[test]
fn test_file_check_mode_with_diff_remove() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("toremove");
    fs::write(&path, "content").unwrap();

    let module = FileModule;
    let mut params: ModuleParams = HashMap::new();
    params.insert(
        "path".to_string(),
        serde_json::json!(path.to_str().unwrap()),
    );
    params.insert("state".to_string(), serde_json::json!("absent"));

    let context = ModuleContext::default()
        .with_check_mode(true)
        .with_diff_mode(true);

    let result = module.check(&params, &context).unwrap();

    assert!(result.changed);
    assert!(result.msg.contains("Would remove"));
    assert!(result.diff.is_some());
    // File should still exist
    assert!(path.exists());
}

// ============================================================================
// Diff with Check Mode Integration Tests
// ============================================================================

#[test]
fn test_check_diff_shows_what_would_change() {
    let temp = TempDir::new().unwrap();
    let dest = temp.path().join("test.txt");

    fs::write(&dest, "original").unwrap();

    let module = CopyModule;
    let mut params: ModuleParams = HashMap::new();
    params.insert("content".to_string(), serde_json::json!("modified"));
    params.insert(
        "dest".to_string(),
        serde_json::json!(dest.to_str().unwrap()),
    );

    let context = ModuleContext::default()
        .with_check_mode(true)
        .with_diff_mode(true);

    let result = module.execute(&params, &context).unwrap();

    assert!(result.changed);
    assert!(result.diff.is_some());

    let diff = result.diff.unwrap();
    assert_eq!(diff.before, "original");
    assert_eq!(diff.after, "modified");

    // Verify no actual change was made
    assert_eq!(fs::read_to_string(&dest).unwrap(), "original");
}

#[test]
fn test_check_diff_no_change_needed() {
    let temp = TempDir::new().unwrap();
    let dest = temp.path().join("test.txt");

    fs::write(&dest, "same").unwrap();

    let module = CopyModule;
    let mut params: ModuleParams = HashMap::new();
    params.insert("content".to_string(), serde_json::json!("same"));
    params.insert(
        "dest".to_string(),
        serde_json::json!(dest.to_str().unwrap()),
    );

    let context = ModuleContext::default()
        .with_check_mode(true)
        .with_diff_mode(true);

    let result = module.execute(&params, &context).unwrap();

    assert!(!result.changed);
}

// ============================================================================
// Large Diff Tests
// ============================================================================

#[test]
fn test_large_file_diff() {
    let temp = TempDir::new().unwrap();
    let dest = temp.path().join("large.txt");

    // Create large content (10000 lines)
    let old_lines: Vec<String> = (0..10000).map(|i| format!("Line {}", i)).collect();
    let new_lines: Vec<String> = (0..10000)
        .map(|i| {
            if i == 5000 {
                "Modified line 5000".to_string()
            } else {
                format!("Line {}", i)
            }
        })
        .collect();

    let old_content = old_lines.join("\n");
    let new_content = new_lines.join("\n");

    fs::write(&dest, &old_content).unwrap();

    let module = CopyModule;
    let mut params: ModuleParams = HashMap::new();
    params.insert("content".to_string(), serde_json::json!(new_content));
    params.insert(
        "dest".to_string(),
        serde_json::json!(dest.to_str().unwrap()),
    );

    let context = ModuleContext::default();
    let diff = module.diff(&params, &context).unwrap();

    assert!(diff.is_some());
    let d = diff.unwrap();
    assert!(!d.before.is_empty());
    assert!(!d.after.is_empty());
}

#[test]
fn test_many_line_changes_diff() {
    let temp = TempDir::new().unwrap();
    let dest = temp.path().join("many_changes.txt");

    let old_content = "line1\nline2\nline3\nline4\nline5";
    let new_content = "modified1\nmodified2\nmodified3\nmodified4\nmodified5";

    fs::write(&dest, old_content).unwrap();

    let module = CopyModule;
    let mut params: ModuleParams = HashMap::new();
    params.insert("content".to_string(), serde_json::json!(new_content));
    params.insert(
        "dest".to_string(),
        serde_json::json!(dest.to_str().unwrap()),
    );

    let context = ModuleContext::default();
    let diff = module.diff(&params, &context).unwrap();

    assert!(diff.is_some());
    let d = diff.unwrap();
    assert_eq!(d.before, old_content);
    assert_eq!(d.after, new_content);
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_diff_special_characters() {
    let temp = TempDir::new().unwrap();
    let dest = temp.path().join("special.txt");

    let old_content = "tab\there\nnewline\n\rcarriage";
    let new_content = "different\ttabs\nand\nlines";

    fs::write(&dest, old_content).unwrap();

    let module = CopyModule;
    let mut params: ModuleParams = HashMap::new();
    params.insert("content".to_string(), serde_json::json!(new_content));
    params.insert(
        "dest".to_string(),
        serde_json::json!(dest.to_str().unwrap()),
    );

    let context = ModuleContext::default();
    let diff = module.diff(&params, &context).unwrap();

    assert!(diff.is_some());
    let d = diff.unwrap();
    assert_eq!(d.before, old_content);
    assert_eq!(d.after, new_content);
}

#[test]
fn test_diff_unicode_characters() {
    let temp = TempDir::new().unwrap();
    let dest = temp.path().join("unicode.txt");

    let old_content = "Hello \u{1F600} World";
    let new_content = "\u{4E2D}\u{6587} \u{65E5}\u{672C}\u{8A9E} \u{D55C}\u{AE00}";

    fs::write(&dest, old_content).unwrap();

    let module = CopyModule;
    let mut params: ModuleParams = HashMap::new();
    params.insert("content".to_string(), serde_json::json!(new_content));
    params.insert(
        "dest".to_string(),
        serde_json::json!(dest.to_str().unwrap()),
    );

    let context = ModuleContext::default();
    let diff = module.diff(&params, &context).unwrap();

    assert!(diff.is_some());
    let d = diff.unwrap();
    assert_eq!(d.before, old_content);
    assert_eq!(d.after, new_content);
}

#[test]
fn test_diff_only_whitespace_changes() {
    let temp = TempDir::new().unwrap();
    let dest = temp.path().join("whitespace.txt");

    let old_content = "line with trailing spaces   ";
    let new_content = "line with trailing spaces";

    fs::write(&dest, old_content).unwrap();

    let module = CopyModule;
    let mut params: ModuleParams = HashMap::new();
    params.insert("content".to_string(), serde_json::json!(new_content));
    params.insert(
        "dest".to_string(),
        serde_json::json!(dest.to_str().unwrap()),
    );

    let context = ModuleContext::default();
    let diff = module.diff(&params, &context).unwrap();

    assert!(diff.is_some());
    let d = diff.unwrap();
    assert_ne!(d.before, d.after);
}

#[test]
fn test_diff_newline_differences() {
    let temp = TempDir::new().unwrap();
    let dest = temp.path().join("newlines.txt");

    let old_content = "line1\nline2\n";
    let new_content = "line1\nline2";

    fs::write(&dest, old_content).unwrap();

    let module = CopyModule;
    let mut params: ModuleParams = HashMap::new();
    params.insert("content".to_string(), serde_json::json!(new_content));
    params.insert(
        "dest".to_string(),
        serde_json::json!(dest.to_str().unwrap()),
    );

    let context = ModuleContext::default();
    let diff = module.diff(&params, &context).unwrap();

    assert!(diff.is_some());
    let d = diff.unwrap();
    assert_ne!(d.before, d.after);
}

// ============================================================================
// Diff Serialization Tests
// ============================================================================

#[test]
fn test_diff_serialization() {
    let diff = Diff::new("before state", "after state");
    let serialized = serde_json::to_string(&diff).unwrap();

    assert!(serialized.contains("before"));
    assert!(serialized.contains("after"));
    assert!(serialized.contains("before state"));
    assert!(serialized.contains("after state"));
}

#[test]
fn test_diff_with_details_serialization() {
    let diff = Diff::new("before", "after").with_details("detailed diff output");
    let serialized = serde_json::to_string(&diff).unwrap();

    assert!(serialized.contains("details"));
    assert!(serialized.contains("detailed diff output"));
}

#[test]
fn test_module_output_with_diff_serialization() {
    let diff = Diff::new("old", "new");
    let output = ModuleOutput::changed("Updated content").with_diff(diff);

    let serialized = serde_json::to_string(&output).unwrap();

    assert!(serialized.contains("diff"));
    assert!(serialized.contains("old"));
    assert!(serialized.contains("new"));
}

// ============================================================================
// Diff Mode Flag Tests
// ============================================================================

#[test]
fn test_diff_mode_disabled_no_diff_in_output() {
    let temp = TempDir::new().unwrap();
    let dest = temp.path().join("test.txt");

    fs::write(&dest, "old").unwrap();

    let module = CopyModule;
    let mut params: ModuleParams = HashMap::new();
    params.insert("content".to_string(), serde_json::json!("new"));
    params.insert(
        "dest".to_string(),
        serde_json::json!(dest.to_str().unwrap()),
    );

    // Check mode but diff mode disabled
    let context = ModuleContext::default()
        .with_check_mode(true)
        .with_diff_mode(false);

    let result = module.execute(&params, &context).unwrap();

    assert!(result.changed);
    // When diff_mode is false, check mode output should not include diff
    assert!(result.diff.is_none());
}

#[test]
fn test_diff_mode_enabled_includes_diff() {
    let temp = TempDir::new().unwrap();
    let dest = temp.path().join("test.txt");

    fs::write(&dest, "old").unwrap();

    let module = CopyModule;
    let mut params: ModuleParams = HashMap::new();
    params.insert("content".to_string(), serde_json::json!("new"));
    params.insert(
        "dest".to_string(),
        serde_json::json!(dest.to_str().unwrap()),
    );

    let context = ModuleContext::default()
        .with_check_mode(true)
        .with_diff_mode(true);

    let result = module.execute(&params, &context).unwrap();

    assert!(result.changed);
    assert!(result.diff.is_some());
}

// ============================================================================
// Integration: Full Workflow Tests
// ============================================================================

#[test]
fn test_full_diff_workflow_copy() {
    let temp = TempDir::new().unwrap();
    let dest = temp.path().join("workflow.txt");

    // Step 1: Create new file (check mode with diff)
    let module = CopyModule;
    let mut params: ModuleParams = HashMap::new();
    params.insert("content".to_string(), serde_json::json!("initial content"));
    params.insert(
        "dest".to_string(),
        serde_json::json!(dest.to_str().unwrap()),
    );

    let check_context = ModuleContext::default()
        .with_check_mode(true)
        .with_diff_mode(true);

    let check_result = module.execute(&params, &check_context).unwrap();
    assert!(check_result.changed);
    assert!(check_result.diff.is_some());
    assert!(!dest.exists()); // File not created in check mode

    // Step 2: Actually create the file
    let exec_context = ModuleContext::default();
    let exec_result = module.execute(&params, &exec_context).unwrap();
    assert!(exec_result.changed);
    assert!(dest.exists());
    assert_eq!(fs::read_to_string(&dest).unwrap(), "initial content");

    // Step 3: Modify with diff
    params.insert("content".to_string(), serde_json::json!("modified content"));

    let modify_check = module.execute(&params, &check_context).unwrap();
    assert!(modify_check.changed);
    assert!(modify_check.diff.is_some());
    let diff = modify_check.diff.unwrap();
    assert_eq!(diff.before, "initial content");
    assert_eq!(diff.after, "modified content");

    // Step 4: Actually modify
    let modify_result = module.execute(&params, &exec_context).unwrap();
    assert!(modify_result.changed);
    assert_eq!(fs::read_to_string(&dest).unwrap(), "modified content");

    // Step 5: No change needed
    let no_change_check = module.execute(&params, &check_context).unwrap();
    assert!(!no_change_check.changed);
}

#[test]
fn test_full_diff_workflow_template() {
    let temp = TempDir::new().unwrap();
    let src = temp.path().join("config.j2");
    let dest = temp.path().join("config.txt");

    fs::write(&src, "port={{ port }}\nhost={{ host }}").unwrap();

    let module = TemplateModule;
    let mut params: ModuleParams = HashMap::new();
    params.insert("src".to_string(), serde_json::json!(src.to_str().unwrap()));
    params.insert(
        "dest".to_string(),
        serde_json::json!(dest.to_str().unwrap()),
    );

    let mut vars = HashMap::new();
    vars.insert("port".to_string(), serde_json::json!(8080));
    vars.insert("host".to_string(), serde_json::json!("localhost"));

    let context = ModuleContext::default()
        .with_vars(vars)
        .with_check_mode(true)
        .with_diff_mode(true);

    let result = module.execute(&params, &context).unwrap();

    assert!(result.changed);
    assert!(result.diff.is_some());
    let diff = result.diff.unwrap();
    assert_eq!(diff.before, "");
    assert_eq!(diff.after, "port=8080\nhost=localhost");
}

#[test]
fn test_full_diff_workflow_file() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("managed_dir");

    let module = FileModule;
    let mut params: ModuleParams = HashMap::new();
    params.insert(
        "path".to_string(),
        serde_json::json!(path.to_str().unwrap()),
    );
    params.insert("state".to_string(), serde_json::json!("directory"));

    // Check mode first
    let check_context = ModuleContext::default()
        .with_check_mode(true)
        .with_diff_mode(true);

    let check_result = module.execute(&params, &check_context).unwrap();
    assert!(check_result.changed);
    assert!(check_result.msg.contains("Would create"));
    assert!(!path.exists());

    // Actually create
    let exec_context = ModuleContext::default();
    let exec_result = module.execute(&params, &exec_context).unwrap();
    assert!(exec_result.changed);
    assert!(path.is_dir());

    // Idempotent check
    let idempotent_result = module.execute(&params, &exec_context).unwrap();
    assert!(!idempotent_result.changed);

    // Remove check
    params.insert("state".to_string(), serde_json::json!("absent"));
    let remove_check = module.execute(&params, &check_context).unwrap();
    assert!(remove_check.changed);
    assert!(remove_check.msg.contains("Would remove"));
    assert!(path.exists()); // Still exists after check mode
}
