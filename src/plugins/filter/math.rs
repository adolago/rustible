//! Math and size-conversion filters for Jinja2 templates.
//!
//! This module provides the numeric filters that Ansible exposes from
//! `ansible.builtin`.
//!
//! # Available Filters
//!
//! - `log`: Logarithm with an optional base (default: natural log)
//! - `pow`: Raise a number to a power
//! - `root`: Nth root of a number (default: square root)
//! - `human_readable`: Format a byte count as a human readable size
//! - `human_to_bytes`: Parse a human readable size into a byte count
//!
//! # Examples
//!
//! ```jinja2
//! {{ 8 | log(2) }}
//! {{ 2 | pow(10) }}
//! {{ 8 | root(3) }}
//! {{ 1269730 | human_readable }}
//! {{ '1.5 GB' | human_to_bytes }}
//! ```

use minijinja::value::Kwargs;
use minijinja::{Environment, Error, ErrorKind, Value};

/// Size unit suffixes in ascending order, 1024 based (Ansible semantics).
const UNITS: [&str; 9] = ["B", "K", "M", "G", "T", "P", "E", "Z", "Y"];

/// Display labels matching Ansible's `bytes_to_human` output.
const BYTE_LABELS: [&str; 9] = ["Bytes", "KB", "MB", "GB", "TB", "PB", "EB", "ZB", "YB"];

/// Display labels used when `isbits` is requested.
const BIT_LABELS: [&str; 9] = ["bits", "Kb", "Mb", "Gb", "Tb", "Pb", "Eb", "Zb", "Yb"];

/// Register all math filters with the given environment.
pub fn register_filters(env: &mut Environment<'static>) {
    env.add_filter("log", log);
    env.add_filter("pow", pow);
    env.add_filter("root", root);
    env.add_filter("human_readable", human_readable);
    env.add_filter("human_to_bytes", human_to_bytes);
}

/// Convert a template value to `f64`, failing loudly like Ansible does.
fn as_number(value: &Value, filter: &str) -> Result<f64, Error> {
    if let Ok(number) = f64::try_from(value.clone()) {
        return Ok(number);
    }
    value
        .as_str()
        .and_then(|text| text.trim().parse::<f64>().ok())
        .ok_or_else(|| {
            Error::new(
                ErrorKind::InvalidOperation,
                format!("{} expects a number, got {}", filter, value.kind()),
            )
        })
}

/// Logarithm of a number.
///
/// # Arguments
///
/// * `input` - The number to take the logarithm of
/// * `base` - Optional base; the natural logarithm is used when omitted
///
/// # Ansible Compatibility
///
/// Compatible with Ansible's `log` filter.
fn log(input: Value, base: Option<Value>) -> Result<f64, Error> {
    let number = as_number(&input, "log")?;
    match base {
        Some(base) => Ok(number.log(as_number(&base, "log")?)),
        None => Ok(number.ln()),
    }
}

/// Raise a number to a power.
///
/// # Ansible Compatibility
///
/// Compatible with Ansible's `pow` filter.
fn pow(input: Value, exponent: Value) -> Result<f64, Error> {
    Ok(as_number(&input, "pow")?.powf(as_number(&exponent, "pow")?))
}

/// Nth root of a number.
///
/// # Arguments
///
/// * `input` - The number to take the root of
/// * `base` - Optional root degree; defaults to 2 (square root)
///
/// # Ansible Compatibility
///
/// Compatible with Ansible's `root` filter.
fn root(input: Value, base: Option<Value>) -> Result<f64, Error> {
    let number = as_number(&input, "root")?;
    let degree = match base {
        Some(base) => as_number(&base, "root")?,
        None => 2.0,
    };
    if degree == 0.0 {
        return Err(Error::new(
            ErrorKind::InvalidOperation,
            "root degree must not be zero",
        ));
    }
    Ok(number.powf(1.0 / degree))
}

/// Format a byte count as a human readable size.
///
/// # Arguments
///
/// * `input` - The size in bytes (or bits when `isbits` is true)
/// * `isbits` - Report bits instead of bytes (first positional, as in Ansible)
/// * `unit` - Optional unit to force, e.g. `"M"` or `"MB"`
///
/// The `precision` keyword argument is a Rustible extension; Ansible always
/// prints two decimal places.
///
/// # Ansible Compatibility
///
/// Argument order matches Ansible's `human_readable(size, isbits, unit)` and
/// units are 1024 based.
fn human_readable(
    input: Value,
    isbits: Option<bool>,
    unit: Option<String>,
    kwargs: Kwargs,
) -> Result<String, Error> {
    let size = as_number(&input, "human_readable")?;
    let precision = kwargs.get::<Option<usize>>("precision")?.unwrap_or(2);
    let isbits = match kwargs.get::<Option<bool>>("isbits")? {
        Some(from_kwargs) => from_kwargs,
        None => isbits.unwrap_or(false),
    };
    let unit = match kwargs.get::<Option<String>>("unit")? {
        Some(from_kwargs) => Some(from_kwargs),
        None => unit,
    };
    kwargs.assert_all_used()?;
    // A lowercase "b" suffix ("Mb") also selects bit labels.
    let isbits = isbits
        || unit
            .as_deref()
            .is_some_and(|unit| unit.len() > 1 && unit.ends_with('b'));
    let labels = if isbits { BIT_LABELS } else { BYTE_LABELS };

    let index = match unit.as_deref() {
        Some(unit) => unit_index(unit).ok_or_else(|| {
            Error::new(
                ErrorKind::InvalidOperation,
                format!("human_readable: unknown unit '{}'", unit),
            )
        })?,
        None => {
            let magnitude = size.abs();
            let mut index = 0;
            while index + 1 < UNITS.len() && magnitude >= 1024_f64.powi(index as i32 + 1) {
                index += 1;
            }
            index
        }
    };

    let scaled = size / 1024_f64.powi(index as i32);
    Ok(format!("{:.*} {}", precision, scaled, labels[index]))
}

/// Parse a human readable size into a byte count.
///
/// # Arguments
///
/// * `input` - The size string, e.g. `"1.5 GB"`, or a bare number
/// * `default_unit` - Optional unit assumed when the input carries none
/// * `isbits` - Treat the value as bits, dividing the result by 8
///
/// # Ansible Compatibility
///
/// Compatible with Ansible's `human_to_bytes` filter: units are 1024 based and
/// an unparsable input is an error rather than a silent zero. A lowercase `b`
/// suffix (`"1Mb"`) is read as bits and divided by 8.
fn human_to_bytes(
    input: Value,
    default_unit: Option<String>,
    isbits: Option<bool>,
) -> Result<i64, Error> {
    let text = match input.as_str() {
        Some(text) => text.trim().to_string(),
        None => as_number(&input, "human_to_bytes")?.to_string(),
    };

    let split = text
        .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-' || c == '+'))
        .unwrap_or(text.len());
    let (number, suffix) = text.split_at(split);

    let number: f64 = number.trim().parse().map_err(|_| {
        Error::new(
            ErrorKind::InvalidOperation,
            format!("human_to_bytes: cannot parse '{}' as a size", text),
        )
    })?;

    let suffix = suffix.trim();
    let unit = if suffix.is_empty() {
        default_unit.unwrap_or_else(|| "B".to_string())
    } else {
        suffix.to_string()
    };

    let index = unit_index(&unit).ok_or_else(|| {
        Error::new(
            ErrorKind::InvalidOperation,
            format!("human_to_bytes: unknown unit '{}'", unit),
        )
    })?;

    let mut bytes = number * 1024_f64.powi(index as i32);
    // "Mb" (lowercase b) denotes bits in Ansible; "MB" and "M" denote bytes.
    // An explicit isbits=true asks for the same division.
    if isbits.unwrap_or(false) || (unit.len() > 1 && unit.ends_with('b')) {
        bytes /= 8.0;
    }

    Ok(bytes.round() as i64)
}

/// Resolve a unit suffix such as `M`, `MB`, `MiB` or `Mb` to its power of 1024.
fn unit_index(unit: &str) -> Option<usize> {
    let unit = unit.trim();
    if unit.is_empty() {
        return Some(0);
    }

    let normalized = unit.to_ascii_uppercase();
    let head = normalized.chars().next()?;
    let tail = &normalized[head.len_utf8()..];
    if !matches!(tail, "" | "B" | "IB") {
        return None;
    }

    // A bare "B" is bytes; every other prefix maps to its 1024 power.
    if head == 'B' {
        return if tail.is_empty() { Some(0) } else { None };
    }

    UNITS
        .iter()
        .position(|candidate| candidate.starts_with(head))
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

    fn render_err(template: &str) -> String {
        env()
            .render_str(template, Value::UNDEFINED)
            .expect_err("template should fail")
            .to_string()
    }

    #[test]
    fn test_log_natural_and_base() {
        assert_eq!(render("{{ (8 | log(2)) | round }}"), "3.0");
        assert!(render("{{ 1 | log }}").starts_with('0'));
    }

    #[test]
    fn test_pow() {
        assert_eq!(render("{{ (2 | pow(10)) | int }}"), "1024");
    }

    #[test]
    fn test_root() {
        assert_eq!(render("{{ (8 | root(3)) | round | int }}"), "2");
        assert_eq!(render("{{ (9 | root) | round | int }}"), "3");
    }

    #[test]
    fn test_root_rejects_zero_degree() {
        assert!(render_err("{{ 9 | root(0) }}").contains("must not be zero"));
    }

    #[test]
    fn test_human_readable() {
        assert_eq!(render("{{ 1024 | human_readable }}"), "1.00 KB");
        assert_eq!(render("{{ 0 | human_readable }}"), "0.00 Bytes");
        assert_eq!(render("{{ 1269730 | human_readable }}"), "1.21 MB");
    }

    #[test]
    fn test_human_readable_precision_and_unit() {
        assert_eq!(
            render("{{ 1269730 | human_readable(precision=0) }}"),
            "1 MB"
        );
        // 1269730 / 1024, matching Ansible's bytes_to_human for unit='K'.
        assert_eq!(
            render("{{ 1269730 | human_readable(unit='K') }}"),
            "1239.97 KB"
        );
        assert_eq!(
            render("{{ 1269730 | human_readable(false, 'K') }}"),
            "1239.97 KB"
        );
        assert_eq!(
            render("{{ 1269730 | human_readable(true) }}"),
            "1.21 Mb",
            "the first positional argument is isbits, as in Ansible"
        );
    }

    #[test]
    fn test_human_to_bytes() {
        assert_eq!(render("{{ '1.5 GB' | human_to_bytes }}"), "1610612736");
        assert_eq!(render("{{ '10M' | human_to_bytes }}"), "10485760");
        assert_eq!(render("{{ '1024' | human_to_bytes }}"), "1024");
        assert_eq!(render("{{ '1 MiB' | human_to_bytes }}"), "1048576");
    }

    #[test]
    fn test_human_to_bytes_bits_are_divided_by_eight() {
        assert_eq!(render("{{ '1Mb' | human_to_bytes }}"), "131072");
    }

    #[test]
    fn test_human_to_bytes_default_unit() {
        assert_eq!(render("{{ '10' | human_to_bytes('M') }}"), "10485760");
    }

    #[test]
    fn test_human_to_bytes_rejects_garbage() {
        assert!(render_err("{{ 'lots' | human_to_bytes }}").contains("cannot parse"));
        assert!(render_err("{{ '10 QB' | human_to_bytes }}").contains("unknown unit"));
    }
}
