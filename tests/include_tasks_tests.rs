//! Integration tests for include_tasks and import_tasks functionality

use rustible::executor::include_handler::IncludeTasksHandler;
use rustible::executor::runtime::RuntimeContext;
use rustible::include::{ImportTasksSpec, IncludeTasksSpec, TaskIncluder};
use rustible::vars::VarStore;
use serde_json::json;
use std::io::Write;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::sync::RwLock;

/// Test basic include_tasks loading
#[tokio::test]
async fn test_basic_include_tasks() {
    let temp_dir = TempDir::new().unwrap();
    let tasks_file = temp_dir.path().join("common_tasks.yml");

    let mut file = std::fs::File::create(&tasks_file).unwrap();
    write!(
        file,
        r#"
- name: Install package
  package:
    name: nginx
    state: present

- name: Start service
  service:
    name: nginx
    state: started
"#
    )
    .unwrap();

    let includer = TaskIncluder::new(temp_dir.path());
    let spec = IncludeTasksSpec::new("common_tasks.yml");
    let var_store = VarStore::new();

    let (tasks, _scope) = includer
        .load_include_tasks(&spec, &var_store)
        .await
        .unwrap();

    assert_eq!(tasks.len(), 2);
    assert_eq!(tasks[0].name, "Install package");
    assert_eq!(tasks[0].module_name(), "package");
    assert_eq!(tasks[1].name, "Start service");
    assert_eq!(tasks[1].module_name(), "service");
}

/// Test include_tasks with variables
#[tokio::test]
async fn test_include_tasks_with_variables() {
    let temp_dir = TempDir::new().unwrap();
    let tasks_file = temp_dir.path().join("tasks_with_vars.yml");

    let mut file = std::fs::File::create(&tasks_file).unwrap();
    write!(
        file,
        r#"
- name: Install package
  package:
    name: "{{ package_name }}"
    state: present

- name: Configure service
  template:
    src: "{{ config_template }}"
    dest: /etc/app/config.yml
"#
    )
    .unwrap();

    let includer = TaskIncluder::new(temp_dir.path());
    let mut spec = IncludeTasksSpec::new("tasks_with_vars.yml");
    spec = spec.with_var("package_name", json!("nginx"));
    spec = spec.with_var("config_template", json!("templates/nginx.conf.j2"));

    let var_store = VarStore::new();

    let (tasks, mut scope) = includer
        .load_include_tasks(&spec, &var_store)
        .await
        .unwrap();

    assert_eq!(tasks.len(), 2);

    // Verify variables are in scope
    assert!(scope.contains("package_name"));
    assert!(scope.contains("config_template"));
}

/// Test import_tasks (static inclusion)
#[tokio::test]
async fn test_basic_import_tasks() {
    let temp_dir = TempDir::new().unwrap();
    let tasks_file = temp_dir.path().join("imported_tasks.yml");

    let mut file = std::fs::File::create(&tasks_file).unwrap();
    write!(
        file,
        r#"
- name: Update cache
  command: apt-get update

- name: Install dependencies
  package:
    name:
      - python3
      - git
    state: present
"#
    )
    .unwrap();

    let includer = TaskIncluder::new(temp_dir.path());
    let spec = ImportTasksSpec::new("imported_tasks.yml");
    let mut var_store = VarStore::new();

    let tasks = includer
        .load_import_tasks(&spec, &mut var_store)
        .await
        .unwrap();

    assert_eq!(tasks.len(), 2);
    assert_eq!(tasks[0].name, "Update cache");
    assert_eq!(tasks[1].name, "Install dependencies");
}

/// Test import_tasks with variable merging
#[tokio::test]
async fn test_import_tasks_variable_merging() {
    let temp_dir = TempDir::new().unwrap();
    let tasks_file = temp_dir.path().join("tasks.yml");

    let mut file = std::fs::File::create(&tasks_file).unwrap();
    write!(
        file,
        r#"
- name: Debug variable
  debug:
    msg: "{{ imported_var }}"
"#
    )
    .unwrap();

    let includer = TaskIncluder::new(temp_dir.path());
    let mut spec = ImportTasksSpec::new("tasks.yml");
    spec = spec.with_var("imported_var", json!("imported_value"));

    let mut var_store = VarStore::new();
    let tasks = includer
        .load_import_tasks(&spec, &mut var_store)
        .await
        .unwrap();

    assert_eq!(tasks.len(), 1);

    // Verify variable was merged into parent scope
    assert!(var_store.contains("imported_var"));
}

/// Test include_tasks with runtime handler
#[tokio::test]
async fn test_include_tasks_handler() {
    let temp_dir = TempDir::new().unwrap();
    let tasks_file = temp_dir.path().join("runtime_tasks.yml");

    let mut file = std::fs::File::create(&tasks_file).unwrap();
    write!(
        file,
        r#"
- name: First task
  debug:
    msg: "Task 1"

- name: Second task
  debug:
    msg: "Task 2"
"#
    )
    .unwrap();

    let handler = IncludeTasksHandler::new(temp_dir.path().to_path_buf());
    let runtime = Arc::new(RwLock::new(RuntimeContext::new()));

    // Initialize runtime with a host
    {
        let mut rt = runtime.write().await;
        rt.add_host("localhost".to_string(), None);
    }

    let spec = IncludeTasksSpec::new("runtime_tasks.yml");
    let tasks = handler
        .load_include_tasks(&spec, &runtime, "localhost")
        .await
        .unwrap();

    assert_eq!(tasks.len(), 2);
    assert_eq!(tasks[0].name, "First task");
    assert_eq!(tasks[1].name, "Second task");
}

/// Test nested include_tasks
#[tokio::test]
async fn test_nested_include_tasks() {
    let temp_dir = TempDir::new().unwrap();

    // Create first level tasks
    let tasks_level1 = temp_dir.path().join("level1.yml");
    let mut file = std::fs::File::create(&tasks_level1).unwrap();
    write!(
        file,
        r#"
- name: Level 1 Task
  debug:
    msg: "This is level 1"
"#
    )
    .unwrap();

    // Create second level tasks (these would be included by level1)
    let tasks_level2 = temp_dir.path().join("level2.yml");
    let mut file = std::fs::File::create(&tasks_level2).unwrap();
    write!(
        file,
        r#"
- name: Level 2 Task
  debug:
    msg: "This is level 2"
"#
    )
    .unwrap();

    let includer = TaskIncluder::new(temp_dir.path());

    // Load level 1
    let spec1 = IncludeTasksSpec::new("level1.yml");
    let var_store = VarStore::new();
    let (tasks1, _) = includer
        .load_include_tasks(&spec1, &var_store)
        .await
        .unwrap();

    assert_eq!(tasks1.len(), 1);
    assert_eq!(tasks1[0].name, "Level 1 Task");

    // Load level 2
    let spec2 = IncludeTasksSpec::new("level2.yml");
    let (tasks2, _) = includer
        .load_include_tasks(&spec2, &var_store)
        .await
        .unwrap();

    assert_eq!(tasks2.len(), 1);
    assert_eq!(tasks2[0].name, "Level 2 Task");
}

/// Test include_tasks with conditional variables
#[tokio::test]
async fn test_include_tasks_conditional_vars() {
    let temp_dir = TempDir::new().unwrap();
    let tasks_file = temp_dir.path().join("conditional_tasks.yml");

    let mut file = std::fs::File::create(&tasks_file).unwrap();
    write!(
        file,
        r#"
- name: Install web server
  package:
    name: "{{ web_server | default('nginx') }}"
    state: present

- name: Configure firewall
  command: ufw allow {{ web_port | default(80) }}
"#
    )
    .unwrap();

    let includer = TaskIncluder::new(temp_dir.path());
    let spec = IncludeTasksSpec::new("conditional_tasks.yml");
    let var_store = VarStore::new();

    let (tasks, _scope) = includer
        .load_include_tasks(&spec, &var_store)
        .await
        .unwrap();

    assert_eq!(tasks.len(), 2);
    assert_eq!(tasks[0].name, "Install web server");
    assert_eq!(tasks[1].name, "Configure firewall");
}

/// Test include_tasks file not found error
#[tokio::test]
async fn test_include_tasks_file_not_found() {
    let temp_dir = TempDir::new().unwrap();
    let includer = TaskIncluder::new(temp_dir.path());

    let spec = IncludeTasksSpec::new("nonexistent.yml");
    let var_store = VarStore::new();

    let result = includer.load_include_tasks(&spec, &var_store).await;
    assert!(result.is_err());
}

/// Test import_tasks with apply_tags
#[tokio::test]
async fn test_import_tasks_with_tags() {
    let temp_dir = TempDir::new().unwrap();
    let tasks_file = temp_dir.path().join("tagged_tasks.yml");

    let mut file = std::fs::File::create(&tasks_file).unwrap();
    write!(
        file,
        r#"
- name: Tagged task
  debug:
    msg: "This task has tags"
  tags:
    - debug
    - test
"#
    )
    .unwrap();

    let includer = TaskIncluder::new(temp_dir.path());
    let spec = ImportTasksSpec::new("tagged_tasks.yml");
    let mut var_store = VarStore::new();

    let tasks = includer
        .load_import_tasks(&spec, &mut var_store)
        .await
        .unwrap();

    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].name, "Tagged task");
    assert_eq!(tasks[0].tags.len(), 2);
    assert!(tasks[0].tags.contains(&"debug".to_string()));
    assert!(tasks[0].tags.contains(&"test".to_string()));
}

/// Test include_vars functionality
#[tokio::test]
async fn test_include_vars() {
    let temp_dir = TempDir::new().unwrap();
    let vars_file = temp_dir.path().join("vars.yml");

    let mut file = std::fs::File::create(&vars_file).unwrap();
    write!(
        file,
        r#"
database_host: localhost
database_port: 5432
database_name: myapp
database_user: appuser
"#
    )
    .unwrap();

    let includer = TaskIncluder::new(temp_dir.path());
    let mut var_store = VarStore::new();

    includer
        .load_vars_from_file("vars.yml", &mut var_store)
        .await
        .unwrap();

    assert!(var_store.contains("database_host"));
    assert!(var_store.contains("database_port"));
    assert!(var_store.contains("database_name"));
    assert!(var_store.contains("database_user"));
}

/// Test include_tasks with complex variable structures
#[tokio::test]
async fn test_include_tasks_complex_vars() {
    let temp_dir = TempDir::new().unwrap();
    let tasks_file = temp_dir.path().join("complex_tasks.yml");

    let mut file = std::fs::File::create(&tasks_file).unwrap();
    write!(
        file,
        r#"
- name: Deploy application
  copy:
    src: "{{ app_config.source }}"
    dest: "{{ app_config.destination }}"
    mode: "{{ app_config.mode | default('0644') }}"
"#
    )
    .unwrap();

    let includer = TaskIncluder::new(temp_dir.path());
    let mut spec = IncludeTasksSpec::new("complex_tasks.yml");

    let app_config = json!({
        "source": "/tmp/app.conf",
        "destination": "/etc/app/app.conf",
        "mode": "0600"
    });

    spec = spec.with_var("app_config", app_config);

    let var_store = VarStore::new();

    let (tasks, mut scope) = includer
        .load_include_tasks(&spec, &var_store)
        .await
        .unwrap();

    assert_eq!(tasks.len(), 1);
    assert!(scope.contains("app_config"));
}
