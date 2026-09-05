//! Bounded inspection of the executable task containers supplied to a policy.

use serde_json::{Map, Value};

const MAX_NODES: usize = 10_000;
const MAX_TASK_DEPTH: usize = 64;
const PLAY_CONTAINERS: [&str; 4] = ["pre_tasks", "tasks", "post_tasks", "handlers"];
const TASK_CONTAINERS: [&str; 3] = ["block", "rescue", "always"];

/// Match the alias contract used by the executor parser and ModuleRegistry.
fn normalize_module_name(name: &str) -> &str {
    name.strip_prefix("ansible.builtin.")
        .or_else(|| name.strip_prefix("ansible.legacy."))
        .map(|suffix| suffix.rsplit('.').next().unwrap_or(suffix))
        .unwrap_or(name)
}

fn external_content(name: &str) -> bool {
    matches!(
        normalize_module_name(name),
        "include_tasks" | "import_tasks" | "include_role" | "import_role" | "import_playbook"
    )
}

// Task/handler metadata in the parser and serialized executor task contracts.
// Arbitrary module arguments and variable mappings are never traversed.
fn task_keyword(key: &str) -> bool {
    matches!(
        key,
        "name"
            | "when"
            | "register"
            | "notify"
            | "listen"
            | "loop"
            | "loop_"
            | "loop_items"
            | "loop_var"
            | "with_items"
            | "with_list"
            | "with_dict"
            | "with_fileglob"
            | "loop_control"
            | "ignore_errors"
            | "ignore_unreachable"
            | "changed_when"
            | "failed_when"
            | "delegate_to"
            | "delegate_facts"
            | "run_once"
            | "tags"
            | "become"
            | "become_user"
            | "become_method"
            | "block"
            | "block_id"
            | "block_role"
            | "rescue"
            | "always"
            | "include_tasks"
            | "import_tasks"
            | "include_role"
            | "import_role"
            | "environment"
            | "retries"
            | "delay"
            | "until"
            | "vars"
            | "module_args"
            | "args"
            | "no_log"
            | "throttle"
            | "any_errors_fatal"
            | "check_mode"
            | "diff"
            | "connection"
            | "async"
            | "async_"
            | "poll"
    )
}

fn visit_list<'a>(
    value: Option<&'a Value>,
    container: &str,
    depth: usize,
    nodes: &mut usize,
    tasks: &mut Vec<&'a Map<String, Value>>,
) -> Result<(), String> {
    let Some(value) = value.filter(|value| !value.is_null()) else {
        return Ok(());
    };
    let list = value
        .as_array()
        .ok_or_else(|| format!("task container '{container}' must be an array or null"))?;
    if list.is_empty() {
        return Ok(());
    }
    if depth > MAX_TASK_DEPTH {
        return Err(format!("task nesting exceeds {MAX_TASK_DEPTH} levels"));
    }
    if list.len() > MAX_NODES - *nodes {
        return Err(format!("play/task traversal exceeds {MAX_NODES} nodes"));
    }
    for value in list {
        if *nodes >= MAX_NODES {
            return Err(format!("play/task traversal exceeds {MAX_NODES} nodes"));
        }
        *nodes += 1;
        let task = value
            .as_object()
            .ok_or_else(|| format!("entry in '{container}' must be a task object"))?;

        // The public Handler serializer wraps its task. Its Task serializer can
        // lose module identity; module_from_task rejects that lossy shape below.
        if container == "handlers" && task.contains_key("task") {
            if task
                .keys()
                .any(|key| !matches!(key.as_str(), "task" | "name" | "listen" | "when"))
            {
                return Err("handler mixes a wrapped task with outer task fields".into());
            }
            let wrapped = task["task"]
                .as_object()
                .ok_or("wrapped handler task must be an object")?;
            if *nodes >= MAX_NODES || depth >= MAX_TASK_DEPTH {
                return Err("wrapped handler exceeds task traversal limits".into());
            }
            *nodes += 1;
            tasks.push(wrapped);
            for field in TASK_CONTAINERS {
                visit_list(wrapped.get(field), field, depth + 2, nodes, tasks)?;
            }
        } else {
            tasks.push(task);
            for field in TASK_CONTAINERS {
                visit_list(task.get(field), field, depth + 1, nodes, tasks)?;
            }
        }
    }
    Ok(())
}

fn visit_play<'a>(
    value: &'a Value,
    nodes: &mut usize,
    tasks: &mut Vec<&'a Map<String, Value>>,
) -> Result<(), String> {
    if *nodes >= MAX_NODES {
        return Err(format!("play/task traversal exceeds {MAX_NODES} nodes"));
    }
    *nodes += 1;
    let play = value.as_object().ok_or("each play must be an object")?;
    if play.keys().any(|key| external_content(key)) {
        return Err(
            "external task/playbook/role content is unsupported for module inspection".into(),
        );
    }
    if let Some(roles) = play.get("roles").filter(|value| !value.is_null()) {
        let roles = roles
            .as_array()
            .ok_or("play roles must be an array or null")?;
        if !roles.is_empty() {
            return Err("external role content is unsupported for module inspection".into());
        }
    }
    for container in PLAY_CONTAINERS {
        visit_list(play.get(container), container, 0, nodes, tasks)?;
    }
    Ok(())
}

fn module_from_task(task: &Map<String, Value>) -> Result<Option<&str>, String> {
    if task.keys().any(|key| external_content(key)) {
        return Err("external task/role content is unsupported for module inspection".into());
    }
    let candidates: Vec<_> = task
        .keys()
        .filter(|key| key.as_str() != "module" && !task_keyword(key))
        .take(2)
        .collect();
    let has_block = task.get("block").is_some_and(Value::is_array);
    if candidates.len() > 1
        || (task.contains_key("module") && !candidates.is_empty())
        || (has_block && (!candidates.is_empty() || task.contains_key("module")))
    {
        return Err("task declares ambiguous module or block fields".into());
    }

    let module = if let Some(module) = task.get("module") {
        module.as_str().ok_or(
            "serialized task module must be a nonempty string; lossy module objects are unsupported",
        )?
    } else if let Some(module) = candidates.first() {
        module.as_str()
    } else if has_block {
        return Ok(None);
    } else {
        // This is the executor parser's behavior for a metadata-only task.
        "debug"
    };
    if normalize_module_name(module).trim().is_empty() {
        return Err("task module name must not be empty".into());
    }
    if external_content(module) {
        return Err("external task/role content is unsupported for module inspection".into());
    }
    Ok(Some(normalize_module_name(module)))
}

pub(super) fn denied_modules(modules: &[&str], input: &Value) -> Result<Vec<String>, String> {
    let mut nodes = 0;
    let mut tasks = Vec::new();
    let plays = match input {
        Value::Array(plays) => Some(plays),
        Value::Object(object) if object.contains_key("plays") => {
            if PLAY_CONTAINERS.iter().any(|key| object.contains_key(*key))
                || object.contains_key("roles")
                || object.keys().any(|key| external_content(key))
            {
                return Err("input mixes a plays wrapper with play content".into());
            }
            Some(object["plays"].as_array().ok_or("plays must be an array")?)
        }
        Value::Object(_) => None,
        _ => return Err("expected an array of plays, a plays wrapper, or a play object".into()),
    };
    if let Some(plays) = plays {
        if plays.len() > MAX_NODES {
            return Err(format!("play/task traversal exceeds {MAX_NODES} nodes"));
        }
        for play in plays {
            visit_play(play, &mut nodes, &mut tasks)?;
        }
    } else {
        visit_play(input, &mut nodes, &mut tasks)?;
    }

    let mut violations = Vec::new();
    for task in tasks {
        if let Some(module) = module_from_task(task)? {
            for denied in modules {
                if module == normalize_module_name(denied) {
                    let name = task
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or("<unnamed>");
                    violations.push(format!("task '{name}' uses denied module '{denied}'"));
                }
            }
        }
    }
    Ok(violations)
}
