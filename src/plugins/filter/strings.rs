//! String and type-inspection filters for Jinja2 templates.
//!
//! Covers the Jinja2 string filters MiniJinja does not provide plus the
//! Ansible-specific string helpers.
//!
//! # Available Filters
//!
//! - `center`: Pad a string to a width, centered
//! - `truncate`: Shorten a string, appending an ellipsis
//! - `wordwrap`: Wrap text at a column
//! - `wordcount`: Count words
//! - `comment`: Wrap text in comment markers
//! - `to_uuid`: Version 5 UUID using Ansible's namespace
//! - `type_debug`: Report the Python-style type name of a value
//!
//! # Examples
//!
//! ```jinja2
//! {{ "Rustible" | center(20) }}
//! {{ long_text | truncate(40) }}
//! {{ notice | comment }}
//! {{ hostname | to_uuid }}
//! ```

use minijinja::value::ValueKind;
use minijinja::{Environment, Error, ErrorKind, Value};

/// Register all string filters with the given environment.
pub fn register_filters(env: &mut Environment<'static>) {
    env.add_filter("center", center);
    env.add_filter("truncate", truncate);
    env.add_filter("wordwrap", wordwrap);
    env.add_filter("wordcount", wordcount);
    env.add_filter("comment", comment);
    env.add_filter("to_uuid", to_uuid);
    env.add_filter("type_debug", type_debug);
}

/// Center a string in a field of the given width.
///
/// # Ansible Compatibility
///
/// Matches Jinja2's `center` filter, which defaults to a width of 80.
fn center(value: Value, width: Option<usize>) -> String {
    let text = to_text(&value);
    let width = width.unwrap_or(80);
    let length = text.chars().count();
    if length >= width {
        return text;
    }
    let total = width - length;
    let left = total / 2;
    let right = total - left;
    format!("{}{}{}", " ".repeat(left), text, " ".repeat(right))
}

/// Truncate a string to at most `length` characters.
///
/// # Arguments
///
/// * `length` - Maximum length including the ellipsis (default 255)
/// * `killwords` - Cut mid-word instead of at a word boundary (default false)
/// * `end` - String appended to a truncated value (default `...`)
/// * `leeway` - Extra characters tolerated before truncating (default 5)
///
/// # Ansible Compatibility
///
/// Matches Jinja2's `truncate` filter, including its `leeway` behavior.
fn truncate(
    value: Value,
    length: Option<usize>,
    killwords: Option<bool>,
    end: Option<String>,
    leeway: Option<usize>,
) -> String {
    let text = to_text(&value);
    let length = length.unwrap_or(255);
    let killwords = killwords.unwrap_or(false);
    let end = end.unwrap_or_else(|| "...".to_string());
    let leeway = leeway.unwrap_or(5);

    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= length + leeway {
        return text;
    }
    let keep = length.saturating_sub(end.chars().count());
    let truncated: String = chars.iter().take(keep).collect();
    if killwords {
        return format!("{}{}", truncated, end);
    }
    // Cut back to the last complete word.
    let trimmed = match truncated.rfind(' ') {
        Some(index) => &truncated[..index],
        None => truncated.as_str(),
    };
    format!("{}{}", trimmed, end)
}

/// Wrap text so that no line exceeds `width` characters.
///
/// # Ansible Compatibility
///
/// Matches Jinja2's `wordwrap` filter (default width 79). Existing newlines
/// are preserved as paragraph breaks.
fn wordwrap(
    value: Value,
    width: Option<usize>,
    break_long_words: Option<bool>,
    wrapstring: Option<String>,
) -> String {
    let text = to_text(&value);
    let width = width.unwrap_or(79).max(1);
    let break_long_words = break_long_words.unwrap_or(true);
    let wrapstring = wrapstring.unwrap_or_else(|| "\n".to_string());

    let mut lines: Vec<String> = Vec::new();
    for paragraph in text.split('\n') {
        let mut current = String::new();
        for word in paragraph.split_whitespace() {
            let mut word = word.to_string();
            if break_long_words {
                // A word longer than the width is split across lines.
                while word.chars().count() > width {
                    if !current.is_empty() {
                        lines.push(std::mem::take(&mut current));
                    }
                    let head: String = word.chars().take(width).collect();
                    lines.push(head);
                    word = word.chars().skip(width).collect();
                }
            }
            if current.is_empty() {
                current = word;
            } else if current.chars().count() + 1 + word.chars().count() <= width {
                current.push(' ');
                current.push_str(&word);
            } else {
                lines.push(std::mem::take(&mut current));
                current = word;
            }
        }
        lines.push(current);
    }

    lines.join(&wrapstring)
}

/// Count the words in a string.
fn wordcount(value: Value) -> usize {
    to_text(&value).split_whitespace().count()
}

/// Wrap text in comment markers.
///
/// # Arguments
///
/// * `style` - `plain` (default), `c`, `cblock`, `erlang` or `xml`
///
/// # Ansible Compatibility
///
/// Matches the default output of Ansible's `comment` filter for its built-in
/// styles. The keyword arguments that override individual markers are not
/// supported and raise an error rather than being ignored.
fn comment(value: Value, style: Option<String>) -> Result<String, Error> {
    let text = to_text(&value);
    let style = style.unwrap_or_else(|| "plain".to_string());

    // (beginning, per-line decoration, end)
    let (beginning, decoration, end) = match style.as_str() {
        "plain" => ("#", "# ", "#"),
        "erlang" => ("%", "% ", "%"),
        "c" => ("//", "// ", "//"),
        "cblock" => ("/*", " * ", " */"),
        "xml" => ("<!--", " - ", "-->"),
        other => {
            return Err(Error::new(
                ErrorKind::InvalidOperation,
                format!(
                    "comment: unsupported style '{}' (supported: plain, erlang, c, cblock, xml)",
                    other
                ),
            ))
        }
    };

    let mut out = String::from(beginning);
    out.push('\n');
    for line in text.split('\n') {
        if line.is_empty() {
            // Keep blank lines blank apart from the trimmed marker.
            out.push_str(decoration.trim_end());
        } else {
            out.push_str(decoration);
            out.push_str(line);
        }
        out.push('\n');
    }
    out.push_str(end);
    Ok(out)
}

/// Produce a version 5 UUID from a string.
///
/// # Ansible Compatibility
///
/// Uses Ansible's namespace UUID (`361E6D51-FAEC-444A-9079-341386DA8E2E`), so
/// the output matches `ansible.builtin.to_uuid` for the same input.
fn to_uuid(value: Value) -> String {
    /// Ansible's UUID namespace, as bytes.
    const NAMESPACE: [u8; 16] = [
        0x36, 0x1E, 0x6D, 0x51, 0xFA, 0xEC, 0x44, 0x4A, 0x90, 0x79, 0x34, 0x13, 0x86, 0xDA, 0x8E,
        0x2E,
    ];

    use sha1::Digest;
    let mut hasher = sha1::Sha1::new();
    hasher.update(NAMESPACE);
    hasher.update(to_text(&value).as_bytes());
    let digest = hasher.finalize();

    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    // Version 5, RFC 4122 variant.
    bytes[6] = (bytes[6] & 0x0f) | 0x50;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;

    let hex: String = bytes.iter().map(|byte| format!("{:02x}", byte)).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

/// Report the type of a value using Python's type names.
///
/// # Ansible Compatibility
///
/// Returns the same names as Ansible's `type_debug`: `str`, `int`, `float`,
/// `bool`, `list`, `dict` and `NoneType`.
fn type_debug(value: Value) -> String {
    match value.kind() {
        ValueKind::Undefined | ValueKind::None => "NoneType",
        ValueKind::Bool => "bool",
        ValueKind::Number => {
            if value.as_i64().is_some() {
                "int"
            } else {
                "float"
            }
        }
        ValueKind::String => "str",
        ValueKind::Bytes => "bytes",
        ValueKind::Seq | ValueKind::Iterable => "list",
        ValueKind::Map => "dict",
        _ => "object",
    }
    .to_string()
}

fn to_text(value: &Value) -> String {
    match value.as_str() {
        Some(text) => text.to_string(),
        None => value.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(template: &str) -> String {
        let mut env = Environment::new();
        register_filters(&mut env);
        env.template_from_str(template)
            .unwrap()
            .render(Value::UNDEFINED)
            .unwrap()
    }

    #[test]
    fn test_center() {
        assert_eq!(render("{{ 'ab' | center(6) }}"), "  ab  ");
        assert_eq!(render("{{ 'abc' | center(6) }}"), " abc  ");
        assert_eq!(render("{{ 'toolong' | center(3) }}"), "toolong");
    }

    #[test]
    fn test_truncate() {
        // Within length + leeway, the string is untouched.
        assert_eq!(render("{{ 'hello world' | truncate(9) }}"), "hello world");
        assert_eq!(
            render("{{ 'hello wonderful world' | truncate(12) }}"),
            "hello..."
        );
        assert_eq!(
            render("{{ 'hello wonderful world' | truncate(12, true) }}"),
            "hello won..."
        );
        assert_eq!(
            render("{{ 'hello wonderful world' | truncate(12, true, '!') }}"),
            "hello wonde!"
        );
    }

    #[test]
    fn test_wordwrap() {
        assert_eq!(
            render("{{ 'one two three four' | wordwrap(8) }}"),
            "one two\nthree\nfour"
        );
        assert_eq!(render("{{ 'aaaaaaaaaa' | wordwrap(4) }}"), "aaaa\naaaa\naa");
        assert_eq!(
            render("{{ 'aaaaaaaaaa' | wordwrap(4, false) }}"),
            "aaaaaaaaaa"
        );
    }

    #[test]
    fn test_wordcount() {
        assert_eq!(render("{{ 'one two  three' | wordcount }}"), "3");
        assert_eq!(render("{{ '' | wordcount }}"), "0");
    }

    #[test]
    fn test_comment_styles() {
        assert_eq!(render("{{ 'notice' | comment }}"), "#\n# notice\n#");
        assert_eq!(render("{{ 'notice' | comment('c') }}"), "//\n// notice\n//");
        assert_eq!(
            render("{{ 'notice' | comment('cblock') }}"),
            "/*\n * notice\n */"
        );
        assert_eq!(
            render("{{ 'notice' | comment('xml') }}"),
            "<!--\n - notice\n-->"
        );
        assert_eq!(
            render("{{ 'first\\nsecond' | comment }}"),
            "#\n# first\n# second\n#"
        );
    }

    #[test]
    fn test_comment_rejects_unknown_style() {
        let mut env = Environment::new();
        register_filters(&mut env);
        let err = env
            .template_from_str("{{ 'x' | comment('lisp') }}")
            .unwrap()
            .render(Value::UNDEFINED)
            .unwrap_err();
        assert!(err.to_string().contains("unsupported style"));
    }

    /// Reference values produced by Python's `uuid.uuid5` with Ansible's
    /// namespace.
    #[test]
    fn test_to_uuid_matches_ansible_namespace() {
        assert_eq!(
            render("{{ 'example' | to_uuid }}"),
            "0cd629ef-c3f7-5d62-98fc-b4270497b261"
        );
        assert_eq!(
            render("{{ 'hello world' | to_uuid }}"),
            "9a129f19-657c-5ca0-80f4-31b29d10569c"
        );
    }

    #[test]
    fn test_type_debug() {
        assert_eq!(render("{{ 'text' | type_debug }}"), "str");
        assert_eq!(render("{{ 1 | type_debug }}"), "int");
        assert_eq!(render("{{ 1.5 | type_debug }}"), "float");
        assert_eq!(render("{{ true | type_debug }}"), "bool");
        assert_eq!(render("{{ [1, 2] | type_debug }}"), "list");
        assert_eq!(render("{{ {'a': 1} | type_debug }}"), "dict");
        assert_eq!(render("{{ none | type_debug }}"), "NoneType");
    }
}
