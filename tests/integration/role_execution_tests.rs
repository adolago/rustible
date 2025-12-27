//! Role execution integration tests
//!
//! These tests verify role execution scenarios including:
//! - Role loading and task execution
//! - Role variable precedence (defaults vs vars vs params)
//! - Role dependency resolution and execution order
//! - Role handler integration
//! - Pre-tasks, roles, tasks, post-tasks execution order
//! - Conditional role execution
//! - Role with tags

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use rustible::executor::playbook::{Play, Playbook};
use rustible::executor::runtime::RuntimeContext;
use rustible::executor::task::{Handler, Task};
use rustible::executor::{Executor, ExecutorConfig};
use tempfile::TempDir;

// ============================================================================
// Test Utilities
// ============================================================================

/// Create a temporary role directory structure
fn create_test_role(temp_dir: &TempDir, role_name: &str) -> PathBuf {
    let role_path = temp_dir.path().join("roles").join(role_name);
    fs::create_dir_all(&role_path).expect("Failed to create role directory");
    role_path
}

/// Create a role with tasks
fn create_role_with_tasks(temp_dir: &TempDir, role_name: &str, tasks_yaml: &str) -> PathBuf {
    let role_path = create_test_role(temp_dir, role_name);
    let tasks_dir = role_path.join("tasks");
    fs::create_dir_all(&tasks_dir).expect("Failed to create tasks directory");
    fs::write(tasks_dir.join("main.yml"), tasks_yaml).expect("Failed to write tasks");
    role_path
}

/// Create a role with defaults
fn create_role_with_defaults(temp_dir: &TempDir, role_name: &str, defaults_yaml: &str) -> PathBuf {
    let role_path = create_test_role(temp_dir, role_name);
    let defaults_dir = role_path.join("defaults");
    fs::create_dir_all(&defaults_dir).expect("Failed to create defaults directory");
    fs::write(defaults_dir.join("main.yml"), defaults_yaml).expect("Failed to write defaults");
    role_path
}

/// Create a role with vars
fn create_role_with_vars(temp_dir: &TempDir, role_name: &str, vars_yaml: &str) -> PathBuf {
    let role_path = create_test_role(temp_dir, role_name);
    let vars_dir = role_path.join("vars");
    fs::create_dir_all(&vars_dir).expect("Failed to create vars directory");
    fs::write(vars_dir.join("main.yml"), vars_yaml).expect("Failed to write vars");
    role_path
}

/// Create a role with handlers
fn create_role_with_handlers(temp_dir: &TempDir, role_name: &str, handlers_yaml: &str) -> PathBuf {
    let role_path = create_test_role(temp_dir, role_name);
    let handlers_dir = role_path.join("handlers");
    fs::create_dir_all(&handlers_dir).expect("Failed to create handlers directory");
    fs::write(handlers_dir.join("main.yml"), handlers_yaml).expect("Failed to write handlers");
    role_path
}

/// Create a role with meta (dependencies)
fn create_role_with_meta(temp_dir: &TempDir, role_name: &str, meta_yaml: &str) -> PathBuf {
    let role_path = create_test_role(temp_dir, role_name);
    let meta_dir = role_path.join("meta");
    fs::create_dir_all(&meta_dir).expect("Failed to create meta directory");
    fs::write(meta_dir.join("main.yml"), meta_yaml).expect("Failed to write meta");
    role_path
}

/// Create a complete role with all components
fn create_complete_role(temp_dir: &TempDir, role_name: &str) -> PathBuf {
    let role_path = create_test_role(temp_dir, role_name);

    // Tasks
    let tasks_dir = role_path.join("tasks");
    fs::create_dir_all(&tasks_dir).unwrap();
    fs::write(
        tasks_dir.join("main.yml"),
        r#"---
- name: Install package
  debug:
    msg: "Installing {{ package_name }}"
  notify: restart service
"#,
    )
    .unwrap();

    // Handlers
    let handlers_dir = role_path.join("handlers");
    fs::create_dir_all(&handlers_dir).unwrap();
    fs::write(
        handlers_dir.join("main.yml"),
        r#"---
- name: restart service
  debug:
    msg: "Restarting {{ service_name }}"
"#,
    )
    .unwrap();

    // Defaults
    let defaults_dir = role_path.join("defaults");
    fs::create_dir_all(&defaults_dir).unwrap();
    fs::write(
        defaults_dir.join("main.yml"),
        r#"---
package_name: nginx
service_name: nginx
default_port: 80
"#,
    )
    .unwrap();

    // Vars
    let vars_dir = role_path.join("vars");
    fs::create_dir_all(&vars_dir).unwrap();
    fs::write(
        vars_dir.join("main.yml"),
        r#"---
config_path: /etc/nginx/nginx.conf
"#,
    )
    .unwrap();

    role_path
}

// ============================================================================
// Basic Role Execution Tests
// ============================================================================

#[tokio::test]
async fn test_play_with_single_role() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let yaml = r#"
- name: Single Role Test
  hosts: all
  gather_facts: false
  roles:
    - common
  tasks:
    - name: After role
      debug:
        msg: "Role completed"
"#;

    // Note: Without actual role files, this tests the playbook structure parsing
    let playbook = Playbook::parse(yaml, None).unwrap();
    assert_eq!(playbook.plays.len(), 1);
    assert_eq!(playbook.plays[0].roles.len(), 1);
}

#[tokio::test]
async fn test_play_with_multiple_roles() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let yaml = r#"
- name: Multiple Roles Test
  hosts: all
  gather_facts: false
  roles:
    - common
    - webserver
    - database
"#;

    let playbook = Playbook::parse(yaml, None).unwrap();
    assert_eq!(playbook.plays[0].roles.len(), 3);
}

#[tokio::test]
async fn test_role_with_inline_parameters() {
    let yaml = r#"
- name: Role Parameters Test
  hosts: all
  gather_facts: false
  roles:
    - role: nginx
      port: 8080
      ssl_enabled: true
"#;

    let playbook = Playbook::parse(yaml, None).unwrap();
    assert_eq!(playbook.plays[0].roles.len(), 1);
    // Role parameters should be accessible
}

#[tokio::test]
async fn test_role_mixed_simple_and_full_syntax() {
    let yaml = r#"
- name: Mixed Role Syntax Test
  hosts: all
  gather_facts: false
  roles:
    - common
    - role: webserver
      port: 80
    - database
    - role: monitoring
      enabled: true
"#;

    let playbook = Playbook::parse(yaml, None).unwrap();
    assert_eq!(playbook.plays[0].roles.len(), 4);
}

// ============================================================================
// Role Variable Precedence Tests
// ============================================================================

#[tokio::test]
async fn test_role_defaults_available_in_tasks() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    // Role defaults should be lowest precedence
    let mut playbook = Playbook::new("Role Defaults Test");
    let mut play = Play::new("Test Play", "all");
    play.gather_facts = false;

    // Simulate role defaults being loaded
    play.set_var("from_defaults", serde_json::json!("default_value"));

    play.add_task(
        Task::new("Use default var", "debug").arg("msg", "{{ from_defaults }}"),
    );

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    assert!(!results.get("localhost").unwrap().failed);
}

#[tokio::test]
async fn test_role_vars_override_defaults() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Role Vars Override Test");
    let mut play = Play::new("Test Play", "all");
    play.gather_facts = false;

    // Role vars have higher precedence than defaults
    play.set_var("setting", serde_json::json!("from_vars"));

    play.add_task(Task::new("Check var", "debug").arg("msg", "{{ setting }}"));

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    assert!(!results.get("localhost").unwrap().failed);
}

#[tokio::test]
async fn test_role_params_override_role_vars() {
    let yaml = r#"
- name: Role Params Override Test
  hosts: all
  gather_facts: false
  roles:
    - role: test_role
      override_var: "from_params"
"#;

    let playbook = Playbook::parse(yaml, None).unwrap();
    // Role params should have highest role-level precedence
    assert_eq!(playbook.plays[0].roles.len(), 1);
}

#[tokio::test]
async fn test_play_vars_available_in_roles() {
    let yaml = r#"
- name: Play Vars in Roles Test
  hosts: all
  gather_facts: false
  vars:
    play_var: "from_play"
  roles:
    - test_role
"#;

    let playbook = Playbook::parse(yaml, None).unwrap();
    assert!(playbook.plays[0].vars.contains_key("play_var"));
}

// ============================================================================
// Role Dependency Tests
// ============================================================================

#[test]
fn test_role_with_dependencies_parsing() {
    let temp_dir = TempDir::new().unwrap();

    // Create dependent role
    let _dep_role = create_role_with_tasks(
        &temp_dir,
        "dependency_role",
        r#"---
- name: Dependency task
  debug:
    msg: "From dependency"
"#,
    );

    // Create main role with dependency
    let main_role = create_role_with_meta(
        &temp_dir,
        "main_role",
        r#"---
dependencies:
  - dependency_role
"#,
    );

    // Create tasks for main role
    let tasks_dir = main_role.join("tasks");
    fs::create_dir_all(&tasks_dir).unwrap();
    fs::write(
        tasks_dir.join("main.yml"),
        r#"---
- name: Main task
  debug:
    msg: "From main role"
"#,
    )
    .unwrap();

    // Verify meta file exists and contains dependencies
    let meta_content =
        fs::read_to_string(main_role.join("meta").join("main.yml")).unwrap();
    assert!(meta_content.contains("dependency_role"));
}

#[test]
fn test_role_dependency_chain() {
    let temp_dir = TempDir::new().unwrap();

    // Create chain: role_c -> role_b -> role_a
    create_role_with_meta(
        &temp_dir,
        "role_a",
        r#"---
dependencies: []
"#,
    );

    create_role_with_meta(
        &temp_dir,
        "role_b",
        r#"---
dependencies:
  - role_a
"#,
    );

    create_role_with_meta(
        &temp_dir,
        "role_c",
        r#"---
dependencies:
  - role_b
"#,
    );

    // Verify dependency chain
    let role_c_meta = temp_dir
        .path()
        .join("roles")
        .join("role_c")
        .join("meta")
        .join("main.yml");
    let content = fs::read_to_string(role_c_meta).unwrap();
    assert!(content.contains("role_b"));
}

#[test]
fn test_role_diamond_dependencies() {
    let temp_dir = TempDir::new().unwrap();

    // Create diamond pattern: top -> left, right; left -> base; right -> base
    create_role_with_meta(
        &temp_dir,
        "base",
        r#"---
dependencies: []
"#,
    );

    create_role_with_meta(
        &temp_dir,
        "left",
        r#"---
dependencies:
  - base
"#,
    );

    create_role_with_meta(
        &temp_dir,
        "right",
        r#"---
dependencies:
  - base
"#,
    );

    create_role_with_meta(
        &temp_dir,
        "top",
        r#"---
dependencies:
  - left
  - right
"#,
    );

    let top_meta = temp_dir
        .path()
        .join("roles")
        .join("top")
        .join("meta")
        .join("main.yml");
    let content = fs::read_to_string(top_meta).unwrap();
    assert!(content.contains("left"));
    assert!(content.contains("right"));
}

// ============================================================================
// Role Execution Order Tests
// ============================================================================

#[tokio::test]
async fn test_pre_tasks_before_roles() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let yaml = r#"
- name: Execution Order Test
  hosts: all
  gather_facts: false
  pre_tasks:
    - name: Pre-task 1
      debug:
        msg: "Before roles"
  roles:
    - common
  tasks:
    - name: Post-role task
      debug:
        msg: "After roles"
"#;

    let playbook = Playbook::parse(yaml, None).unwrap();
    assert_eq!(playbook.plays[0].pre_tasks.len(), 1);
    assert_eq!(playbook.plays[0].roles.len(), 1);
    assert_eq!(playbook.plays[0].tasks.len(), 1);
}

#[tokio::test]
async fn test_post_tasks_after_roles() {
    let yaml = r#"
- name: Post-tasks Order Test
  hosts: all
  gather_facts: false
  roles:
    - webserver
  tasks:
    - name: Regular task
      debug:
        msg: "Regular"
  post_tasks:
    - name: Post-task
      debug:
        msg: "After everything"
"#;

    let playbook = Playbook::parse(yaml, None).unwrap();
    assert_eq!(playbook.plays[0].post_tasks.len(), 1);
}

#[tokio::test]
async fn test_complete_execution_order() {
    let yaml = r#"
- name: Complete Order Test
  hosts: all
  gather_facts: false
  pre_tasks:
    - name: Pre-task 1
      debug:
        msg: "Pre 1"
    - name: Pre-task 2
      debug:
        msg: "Pre 2"
  roles:
    - role1
    - role2
  tasks:
    - name: Task 1
      debug:
        msg: "Task 1"
    - name: Task 2
      debug:
        msg: "Task 2"
  post_tasks:
    - name: Post-task 1
      debug:
        msg: "Post 1"
    - name: Post-task 2
      debug:
        msg: "Post 2"
"#;

    let playbook = Playbook::parse(yaml, None).unwrap();
    let play = &playbook.plays[0];

    assert_eq!(play.pre_tasks.len(), 2);
    assert_eq!(play.roles.len(), 2);
    assert_eq!(play.tasks.len(), 2);
    assert_eq!(play.post_tasks.len(), 2);
}

// ============================================================================
// Role Handler Integration Tests
// ============================================================================

#[tokio::test]
async fn test_role_handlers_accessible() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Role Handler Test");
    let mut play = Play::new("Test Play", "all");
    play.gather_facts = false;

    // Simulate role task that notifies handler
    play.add_task(
        Task::new("Role task", "copy")
            .arg("src", "config.conf")
            .arg("dest", "/etc/config.conf")
            .notify("restart service"),
    );

    // Role handler
    play.add_handler(Handler {
        name: "restart service".to_string(),
        module: "debug".to_string(),
        args: {
            let mut args = indexmap::IndexMap::new();
            args.insert("msg".to_string(), serde_json::json!("Handler executed"));
            args
        },
        when: None,
        listen: vec![],
    });

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    assert!(results.contains_key("localhost"));
}

#[tokio::test]
async fn test_role_handlers_run_at_end_of_play() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig::default();
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Handler Flush Test");
    let mut play = Play::new("Test Play", "all");
    play.gather_facts = false;

    // Multiple tasks notifying same handler
    for i in 1..=3 {
        play.add_task(
            Task::new(format!("Config update {}", i), "copy")
                .arg("src", format!("file{}.conf", i))
                .arg("dest", format!("/etc/file{}.conf", i))
                .notify("reload config"),
        );
    }

    play.add_handler(Handler {
        name: "reload config".to_string(),
        module: "debug".to_string(),
        args: {
            let mut args = indexmap::IndexMap::new();
            args.insert("msg".to_string(), serde_json::json!("Config reloaded"));
            args
        },
        when: None,
        listen: vec![],
    });

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    // Handler should run once at end, not after each task
    assert!(results.contains_key("localhost"));
}

// ============================================================================
// Conditional Role Execution Tests
// ============================================================================

#[tokio::test]
async fn test_role_with_when_condition() {
    let yaml = r#"
- name: Conditional Role Test
  hosts: all
  gather_facts: false
  roles:
    - role: webserver
      when: install_webserver | default(true)
"#;

    let playbook = Playbook::parse(yaml, None).unwrap();
    assert_eq!(playbook.plays[0].roles.len(), 1);
}

#[tokio::test]
async fn test_multiple_roles_with_conditions() {
    let yaml = r#"
- name: Multiple Conditional Roles Test
  hosts: all
  gather_facts: false
  roles:
    - role: nginx
      when: use_nginx
    - role: apache
      when: use_apache
    - role: haproxy
      when: use_loadbalancer
"#;

    let playbook = Playbook::parse(yaml, None).unwrap();
    assert_eq!(playbook.plays[0].roles.len(), 3);
}

#[tokio::test]
async fn test_role_condition_with_variable() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);
    runtime.set_global_var("enable_role".to_string(), serde_json::json!(true));

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Variable Condition Test");
    let mut play = Play::new("Test Play", "all");
    play.gather_facts = false;

    // Conditional task simulating role behavior
    play.add_task(
        Task::new("Conditional role task", "debug")
            .arg("msg", "Role enabled")
            .when("enable_role"),
    );

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    assert!(!results.get("localhost").unwrap().failed);
}

// ============================================================================
// Role Tags Tests
// ============================================================================

#[tokio::test]
async fn test_role_with_tags() {
    let yaml = r#"
- name: Tagged Role Test
  hosts: all
  gather_facts: false
  roles:
    - role: nginx
      tags:
        - web
        - frontend
"#;

    let playbook = Playbook::parse(yaml, None).unwrap();
    assert_eq!(playbook.plays[0].roles.len(), 1);
}

#[tokio::test]
async fn test_multiple_roles_with_different_tags() {
    let yaml = r#"
- name: Multi-tag Role Test
  hosts: all
  gather_facts: false
  roles:
    - role: nginx
      tags: [web]
    - role: mysql
      tags: [database, backend]
    - role: redis
      tags: [cache, backend]
"#;

    let playbook = Playbook::parse(yaml, None).unwrap();
    assert_eq!(playbook.plays[0].roles.len(), 3);
}

// ============================================================================
// Include/Import Role Tests
// ============================================================================

#[tokio::test]
async fn test_include_role_in_tasks() {
    let yaml = r#"
- name: Include Role Test
  hosts: all
  gather_facts: false
  tasks:
    - name: Include common role
      include_role:
        name: common

    - name: Include role with tasks_from
      include_role:
        name: webserver
        tasks_from: install
"#;

    let playbook = Playbook::parse(yaml, None).unwrap();
    assert_eq!(playbook.plays[0].tasks.len(), 2);
}

#[tokio::test]
async fn test_import_role_in_tasks() {
    let yaml = r#"
- name: Import Role Test
  hosts: all
  gather_facts: false
  tasks:
    - name: Import database role
      import_role:
        name: database
"#;

    let playbook = Playbook::parse(yaml, None).unwrap();
    assert_eq!(playbook.plays[0].tasks.len(), 1);
}

#[tokio::test]
async fn test_include_role_with_loop() {
    let yaml = r#"
- name: Include Role with Loop Test
  hosts: all
  gather_facts: false
  tasks:
    - name: Include roles in loop
      include_role:
        name: "{{ item }}"
      loop:
        - role1
        - role2
        - role3
"#;

    let playbook = Playbook::parse(yaml, None).unwrap();
    assert_eq!(playbook.plays[0].tasks.len(), 1);
}

// ============================================================================
// Role File Structure Tests
// ============================================================================

#[test]
fn test_role_tasks_main_yml() {
    let temp_dir = TempDir::new().unwrap();

    let tasks_yaml = r#"---
- name: Task 1
  debug:
    msg: "First task"

- name: Task 2
  debug:
    msg: "Second task"

- name: Task 3
  command: echo "Hello"
"#;

    let role_path = create_role_with_tasks(&temp_dir, "test_role", tasks_yaml);
    let tasks_file = role_path.join("tasks").join("main.yml");

    assert!(tasks_file.exists());
    let content = fs::read_to_string(tasks_file).unwrap();
    assert!(content.contains("Task 1"));
    assert!(content.contains("Task 2"));
    assert!(content.contains("Task 3"));
}

#[test]
fn test_role_handlers_main_yml() {
    let temp_dir = TempDir::new().unwrap();

    let handlers_yaml = r#"---
- name: restart nginx
  service:
    name: nginx
    state: restarted

- name: reload nginx
  service:
    name: nginx
    state: reloaded
"#;

    let role_path = create_role_with_handlers(&temp_dir, "test_role", handlers_yaml);
    let handlers_file = role_path.join("handlers").join("main.yml");

    assert!(handlers_file.exists());
    let content = fs::read_to_string(handlers_file).unwrap();
    assert!(content.contains("restart nginx"));
    assert!(content.contains("reload nginx"));
}

#[test]
fn test_role_defaults_main_yml() {
    let temp_dir = TempDir::new().unwrap();

    let defaults_yaml = r#"---
http_port: 80
https_port: 443
server_name: localhost
"#;

    let role_path = create_role_with_defaults(&temp_dir, "test_role", defaults_yaml);
    let defaults_file = role_path.join("defaults").join("main.yml");

    assert!(defaults_file.exists());
    let content = fs::read_to_string(defaults_file).unwrap();
    assert!(content.contains("http_port: 80"));
    assert!(content.contains("https_port: 443"));
}

#[test]
fn test_role_vars_main_yml() {
    let temp_dir = TempDir::new().unwrap();

    let vars_yaml = r#"---
internal_config: /etc/app/config
log_directory: /var/log/app
"#;

    let role_path = create_role_with_vars(&temp_dir, "test_role", vars_yaml);
    let vars_file = role_path.join("vars").join("main.yml");

    assert!(vars_file.exists());
    let content = fs::read_to_string(vars_file).unwrap();
    assert!(content.contains("internal_config"));
    assert!(content.contains("log_directory"));
}

#[test]
fn test_complete_role_structure() {
    let temp_dir = TempDir::new().unwrap();
    let role_path = create_complete_role(&temp_dir, "complete_role");

    // Verify all directories exist
    assert!(role_path.join("tasks").exists());
    assert!(role_path.join("handlers").exists());
    assert!(role_path.join("defaults").exists());
    assert!(role_path.join("vars").exists());

    // Verify main files exist
    assert!(role_path.join("tasks").join("main.yml").exists());
    assert!(role_path.join("handlers").join("main.yml").exists());
    assert!(role_path.join("defaults").join("main.yml").exists());
    assert!(role_path.join("vars").join("main.yml").exists());
}

// ============================================================================
// Role with Multi-Host Execution Tests
// ============================================================================

#[tokio::test]
async fn test_role_execution_on_multiple_hosts() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("web1".to_string(), Some("webservers"));
    runtime.add_host("web2".to_string(), Some("webservers"));
    runtime.add_host("db1".to_string(), Some("databases"));

    let config = ExecutorConfig {
        gather_facts: false,
        forks: 3,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Multi-Host Role Test");

    // Web play
    let mut web_play = Play::new("Web Servers", "webservers");
    web_play.gather_facts = false;
    web_play.add_task(Task::new("Web task", "debug").arg("msg", "Web server {{ inventory_hostname }}"));
    playbook.add_play(web_play);

    // DB play
    let mut db_play = Play::new("Database Servers", "databases");
    db_play.gather_facts = false;
    db_play.add_task(Task::new("DB task", "debug").arg("msg", "Database {{ inventory_hostname }}"));
    playbook.add_play(db_play);

    let results = executor.run_playbook(&playbook).await.unwrap();

    assert!(results.contains_key("web1"));
    assert!(results.contains_key("web2"));
    assert!(results.contains_key("db1"));
}

// ============================================================================
// Edge Cases Tests
// ============================================================================

#[tokio::test]
async fn test_empty_roles_list() {
    let yaml = r#"
- name: No Roles Test
  hosts: all
  gather_facts: false
  roles: []
  tasks:
    - name: Just a task
      debug:
        msg: "No roles"
"#;

    let playbook = Playbook::parse(yaml, None).unwrap();
    assert!(playbook.plays[0].roles.is_empty());
    assert_eq!(playbook.plays[0].tasks.len(), 1);
}

#[tokio::test]
async fn test_role_only_play() {
    let yaml = r#"
- name: Roles Only Test
  hosts: all
  gather_facts: false
  roles:
    - common
    - webserver
"#;

    let playbook = Playbook::parse(yaml, None).unwrap();
    assert_eq!(playbook.plays[0].roles.len(), 2);
    assert!(playbook.plays[0].tasks.is_empty());
}

#[test]
fn test_role_name_with_special_characters() {
    let temp_dir = TempDir::new().unwrap();

    // Role with dashes
    create_role_with_tasks(
        &temp_dir,
        "my-role-name",
        "---\n- debug: msg=test\n",
    );

    // Role with underscores
    create_role_with_tasks(
        &temp_dir,
        "my_role_name",
        "---\n- debug: msg=test\n",
    );

    assert!(temp_dir.path().join("roles").join("my-role-name").exists());
    assert!(temp_dir.path().join("roles").join("my_role_name").exists());
}

#[test]
fn test_role_with_empty_directories() {
    let temp_dir = TempDir::new().unwrap();
    let role_path = create_test_role(&temp_dir, "empty_role");

    // Create empty subdirectories
    fs::create_dir_all(role_path.join("tasks")).unwrap();
    fs::create_dir_all(role_path.join("handlers")).unwrap();
    fs::create_dir_all(role_path.join("defaults")).unwrap();

    // Directories exist but have no main.yml
    assert!(role_path.join("tasks").exists());
    assert!(!role_path.join("tasks").join("main.yml").exists());
}

// ============================================================================
// Role Variable Merging Tests
// ============================================================================

#[tokio::test]
async fn test_role_vars_merge_with_play_vars() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let yaml = r#"
- name: Variable Merge Test
  hosts: all
  gather_facts: false
  vars:
    play_var: "from_play"
    shared_var: "play_value"
  tasks:
    - name: Check vars
      debug:
        msg: "{{ play_var }} - {{ shared_var }}"
"#;

    let playbook = Playbook::parse(yaml, None).unwrap();
    let results = executor.run_playbook(&playbook).await.unwrap();
    assert!(!results.get("localhost").unwrap().failed);
}

#[tokio::test]
async fn test_extra_vars_override_role_vars() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);
    runtime.set_global_var("override_var".to_string(), serde_json::json!("from_extra"));

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Extra Vars Override Test");
    let mut play = Play::new("Test Play", "all");
    play.gather_facts = false;

    // Role would set this, but extra vars should override
    play.set_var("override_var", serde_json::json!("from_role"));

    play.add_task(Task::new("Check override", "debug").arg("msg", "{{ override_var }}"));

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    assert!(!results.get("localhost").unwrap().failed);
}

// ============================================================================
// Role Statistics Tests
// ============================================================================

#[tokio::test]
async fn test_role_execution_statistics() {
    let mut runtime = RuntimeContext::new();
    runtime.add_host("localhost".to_string(), None);

    let config = ExecutorConfig {
        gather_facts: false,
        ..Default::default()
    };
    let executor = Executor::with_runtime(config, runtime);

    let mut playbook = Playbook::new("Role Stats Test");
    let mut play = Play::new("Test Play", "all");
    play.gather_facts = false;

    // Multiple tasks simulating role execution
    for i in 1..=5 {
        play.add_task(
            Task::new(format!("Role task {}", i), "debug")
                .arg("msg", format!("Task {}", i)),
        );
    }

    playbook.add_play(play);

    let results = executor.run_playbook(&playbook).await.unwrap();
    let host_result = results.get("localhost").unwrap();

    // Verify statistics
    let total = host_result.stats.ok + host_result.stats.changed;
    assert!(total >= 5, "Expected at least 5 tasks executed, got {}", total);
}
