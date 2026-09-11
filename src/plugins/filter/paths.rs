//! Path manipulation filters for Jinja2 templates.
//!
//! This module provides the path-oriented filters that Ansible exposes from
//! `ansible.builtin`, including the Windows-specific variants that operate on
//! backslash-separated paths regardless of the controller platform.
//!
//! # Available Filters
//!
//! - `path_join`: Join path segments into a single path
//! - `splitext`: Split a path into its root and extension
//! - `relpath`: Compute a path relative to a start directory
//! - `expandvars`: Expand environment variables in a path
//! - `win_basename`: Final component of a Windows path
//! - `win_dirname`: Directory component of a Windows path
//! - `win_splitdrive`: Split a Windows path into drive and remainder
//!
//! # Examples
//!
//! ```jinja2
//! {{ ['/etc', 'nginx', 'nginx.conf'] | path_join }}
//! {{ '/etc/nginx/nginx.conf' | splitext }}
//! {{ '/etc/nginx/nginx.conf' | relpath('/etc') }}
//! {{ '$HOME/bin' | expandvars }}
//! {{ 'C:\\Windows\\System32\\cmd.exe' | win_basename }}
//! ```

use minijinja::value::ValueKind;
use minijinja::{Environment, Value};
use std::path::{Component, Path, PathBuf};

/// Register all path filters with the given environment.
pub fn register_filters(env: &mut Environment<'static>) {
    env.add_filter("path_join", path_join);
    env.add_filter("splitext", splitext);
    env.add_filter("relpath", relpath);
    env.add_filter("expandvars", expandvars);
    env.add_filter("win_basename", win_basename);
    env.add_filter("win_dirname", win_dirname);
    env.add_filter("win_splitdrive", win_splitdrive);
}

/// Join path segments into a single path.
///
/// # Arguments
///
/// * `input` - A single path segment or a sequence of segments
///
/// # Returns
///
/// The joined path. As with `os.path.join`, an absolute segment discards
/// everything joined before it.
///
/// # Ansible Compatibility
///
/// Compatible with Ansible's `path_join` filter.
fn path_join(input: Value) -> String {
    let segments: Vec<String> = match input.kind() {
        ValueKind::Seq | ValueKind::Iterable => match input.try_iter() {
            Ok(iter) => iter.map(|item| item.to_string()).collect(),
            Err(_) => return input.to_string(),
        },
        _ => vec![input.to_string()],
    };

    let mut joined = PathBuf::new();
    for segment in segments {
        // PathBuf::push already resets on absolute segments, matching
        // os.path.join semantics.
        joined.push(segment);
    }
    joined.to_string_lossy().into_owned()
}

/// Split a path into `[root, extension]`.
///
/// # Ansible Compatibility
///
/// Compatible with Ansible's `splitext` filter, which returns a two element
/// tuple. Rustible returns a two element list, which renders and indexes the
/// same way in templates.
fn splitext(input: String) -> Value {
    let path = Path::new(&input);
    match path.extension().and_then(|ext| ext.to_str()) {
        // A leading dot with no stem (".bashrc") is not an extension.
        Some(ext) if path.file_stem().is_some_and(|stem| !stem.is_empty()) => {
            let root_len = input.len() - ext.len() - 1;
            Value::from(vec![
                Value::from(input[..root_len].to_string()),
                Value::from(format!(".{}", ext)),
            ])
        }
        _ => Value::from(vec![Value::from(input), Value::from(String::new())]),
    }
}

/// Compute `input` relative to the `start` directory.
///
/// # Ansible Compatibility
///
/// Compatible with Ansible's `relpath` filter. Both paths are treated
/// lexically; the filesystem is not consulted.
fn relpath(input: String, start: String) -> String {
    let target = normalize(Path::new(&input));
    let base = normalize(Path::new(&start));

    if target == base {
        return ".".to_string();
    }

    let mut target_parts: Vec<&std::ffi::OsStr> =
        target.components().map(|c| c.as_os_str()).collect();
    let mut base_parts: Vec<&std::ffi::OsStr> = base.components().map(|c| c.as_os_str()).collect();

    let common = target_parts
        .iter()
        .zip(base_parts.iter())
        .take_while(|(a, b)| a == b)
        .count();

    target_parts.drain(..common);
    base_parts.drain(..common);

    let mut result = PathBuf::new();
    for _ in 0..base_parts.len() {
        result.push("..");
    }
    for part in target_parts {
        result.push(part);
    }

    if result.as_os_str().is_empty() {
        ".".to_string()
    } else {
        result.to_string_lossy().into_owned()
    }
}

/// Resolve `.` and `..` components without touching the filesystem.
fn normalize(path: &Path) -> PathBuf {
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !result.pop() {
                    result.push("..");
                }
            }
            other => result.push(other.as_os_str()),
        }
    }
    result
}

/// Expand `$VAR` and `${VAR}` references from the controller environment.
///
/// # Ansible Compatibility
///
/// Compatible with Ansible's `expandvars` filter. Unknown variables are left
/// untouched, matching `os.path.expandvars`.
fn expandvars(input: String) -> String {
    let bytes: Vec<char> = input.chars().collect();
    let mut result = String::with_capacity(input.len());
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] != '$' {
            result.push(bytes[index]);
            index += 1;
            continue;
        }

        let (name, consumed) = match bytes.get(index + 1) {
            Some('{') => {
                match bytes[index + 2..].iter().position(|c| *c == '}') {
                    Some(offset) => {
                        let name: String = bytes[index + 2..index + 2 + offset].iter().collect();
                        (name, offset + 3)
                    }
                    // Unterminated "${" is literal, as in os.path.expandvars.
                    None => (String::new(), 0),
                }
            }
            Some(c) if c.is_alphanumeric() || *c == '_' => {
                let len = bytes[index + 1..]
                    .iter()
                    .take_while(|c| c.is_alphanumeric() || **c == '_')
                    .count();
                let name: String = bytes[index + 1..index + 1 + len].iter().collect();
                (name, len + 1)
            }
            _ => (String::new(), 0),
        };

        if consumed == 0 {
            result.push('$');
            index += 1;
            continue;
        }

        match std::env::var(&name) {
            Ok(value) => result.push_str(&value),
            Err(_) => result.extend(bytes[index..index + consumed].iter()),
        }
        index += consumed;
    }

    result
}

/// Split a Windows path into `(drive, remainder)`.
fn split_windows_drive(path: &str) -> (&str, &str) {
    let chars: Vec<char> = path.chars().take(2).collect();
    if chars.len() == 2 && chars[1] == ':' && chars[0].is_ascii_alphabetic() {
        path.split_at(2)
    } else {
        ("", path)
    }
}

/// Final component of a Windows path.
///
/// # Ansible Compatibility
///
/// Compatible with Ansible's `win_basename` filter. Both `\` and `/` are
/// treated as separators, and the controller platform is irrelevant.
fn win_basename(input: String) -> String {
    let (_, rest) = split_windows_drive(&input);
    match rest.rfind(['\\', '/']) {
        Some(index) => rest[index + 1..].to_string(),
        None => rest.to_string(),
    }
}

/// Directory component of a Windows path.
///
/// # Ansible Compatibility
///
/// Compatible with Ansible's `win_dirname` filter.
fn win_dirname(input: String) -> String {
    let (drive, rest) = split_windows_drive(&input);
    match rest.rfind(['\\', '/']) {
        Some(0) => format!("{}{}", drive, &rest[..1]),
        Some(index) => {
            let trimmed = rest[..index].trim_end_matches(['\\', '/']);
            if trimmed.is_empty() {
                format!("{}{}", drive, &rest[..index])
            } else {
                format!("{}{}", drive, trimmed)
            }
        }
        None => drive.to_string(),
    }
}

/// Split a Windows path into `[drive, remainder]`.
///
/// # Ansible Compatibility
///
/// Compatible with Ansible's `win_splitdrive` filter, which returns a tuple.
/// Rustible returns a two element list.
fn win_splitdrive(input: String) -> Value {
    let (drive, rest) = split_windows_drive(&input);
    Value::from(vec![
        Value::from(drive.to_string()),
        Value::from(rest.to_string()),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env() -> Environment<'static> {
        let mut env = Environment::new();
        register_filters(&mut env);
        env
    }

    fn render(template: &str) -> String {
        env().render_str(template, Value::UNDEFINED).unwrap()
    }

    #[test]
    fn test_path_join_sequence() {
        assert_eq!(
            render("{{ ['/etc', 'nginx', 'nginx.conf'] | path_join }}"),
            "/etc/nginx/nginx.conf"
        );
    }

    #[test]
    fn test_path_join_absolute_segment_resets() {
        assert_eq!(
            render("{{ ['/etc', '/var', 'log'] | path_join }}"),
            "/var/log"
        );
    }

    #[test]
    fn test_path_join_single_string() {
        assert_eq!(render("{{ '/etc/hosts' | path_join }}"), "/etc/hosts");
    }

    #[test]
    fn test_splitext() {
        assert_eq!(
            render("{{ ('/etc/nginx/nginx.conf' | splitext)[0] }}"),
            "/etc/nginx/nginx"
        );
        assert_eq!(
            render("{{ ('/etc/nginx/nginx.conf' | splitext)[1] }}"),
            ".conf"
        );
    }

    #[test]
    fn test_splitext_without_extension() {
        assert_eq!(render("{{ ('/etc/hosts' | splitext)[1] }}"), "");
    }

    #[test]
    fn test_splitext_dotfile_has_no_extension() {
        assert_eq!(
            render("{{ ('/home/me/.bashrc' | splitext)[0] }}"),
            "/home/me/.bashrc"
        );
        assert_eq!(render("{{ ('/home/me/.bashrc' | splitext)[1] }}"), "");
    }

    #[test]
    fn test_relpath() {
        assert_eq!(
            render("{{ '/etc/nginx/nginx.conf' | relpath('/etc') }}"),
            "nginx/nginx.conf"
        );
    }

    #[test]
    fn test_relpath_walks_up() {
        assert_eq!(
            render("{{ '/var/log' | relpath('/var/lib/app') }}"),
            "../../log"
        );
    }

    #[test]
    fn test_relpath_same_path() {
        assert_eq!(render("{{ '/etc' | relpath('/etc') }}"), ".");
    }

    #[test]
    fn test_expandvars() {
        std::env::set_var("RUSTIBLE_EXPANDVARS_TEST", "/opt/rustible");
        assert_eq!(
            render("{{ '$RUSTIBLE_EXPANDVARS_TEST/bin' | expandvars }}"),
            "/opt/rustible/bin"
        );
        assert_eq!(
            render("{{ '${RUSTIBLE_EXPANDVARS_TEST}/bin' | expandvars }}"),
            "/opt/rustible/bin"
        );
        std::env::remove_var("RUSTIBLE_EXPANDVARS_TEST");
    }

    #[test]
    fn test_expandvars_unknown_variable_is_left_alone() {
        assert_eq!(
            render("{{ '$RUSTIBLE_NOT_SET_VARIABLE/x' | expandvars }}"),
            "$RUSTIBLE_NOT_SET_VARIABLE/x"
        );
    }

    #[test]
    fn test_win_basename() {
        assert_eq!(
            render(r"{{ 'C:\\Windows\\System32\\cmd.exe' | win_basename }}"),
            "cmd.exe"
        );
        assert_eq!(render(r"{{ 'cmd.exe' | win_basename }}"), "cmd.exe");
    }

    #[test]
    fn test_win_dirname() {
        assert_eq!(
            render(r"{{ 'C:\\Windows\\System32\\cmd.exe' | win_dirname }}"),
            r"C:\Windows\System32"
        );
        assert_eq!(render(r"{{ 'C:\\file.txt' | win_dirname }}"), r"C:\");
    }

    #[test]
    fn test_win_splitdrive() {
        assert_eq!(
            render(r"{{ ('C:\\Windows\\file.txt' | win_splitdrive)[0] }}"),
            "C:"
        );
        assert_eq!(
            render(r"{{ ('C:\\Windows\\file.txt' | win_splitdrive)[1] }}"),
            r"\Windows\file.txt"
        );
        assert_eq!(
            render(r"{{ ('\\\\server\\share' | win_splitdrive)[0] }}"),
            ""
        );
    }
}
