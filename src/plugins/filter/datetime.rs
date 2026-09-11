//! Date and time filters for Jinja2 templates.
//!
//! # Available Filters
//!
//! - `strftime`: Format a timestamp with a strftime pattern
//! - `to_datetime`: Parse a timestamp string into a normalized form
//!
//! # Examples
//!
//! ```jinja2
//! {{ '%Y-%m-%d' | strftime }}
//! {{ '%Y-%m-%d' | strftime(1666011400) }}
//! {{ '12/12/2019' | to_datetime('%m/%d/%Y') }}
//! ```
//!
//! # Ansible Compatibility
//!
//! `strftime` matches Ansible: the format string is the filter input and the
//! optional argument is the epoch second to format.
//!
//! `to_datetime` differs in its return type. Ansible returns a Python
//! `datetime` object that supports arithmetic; Rustible returns the normalized
//! `%Y-%m-%d %H:%M:%S` string, because MiniJinja has no datetime value type.
//! Use `strftime` for formatting and epoch arithmetic instead.

use chrono::{DateTime, Local, NaiveDateTime, TimeZone, Utc};
use minijinja::{Environment, Error, ErrorKind, Value};

/// Ansible's default input format for `to_datetime`.
const DEFAULT_DATETIME_FORMAT: &str = "%Y-%m-%d %H:%M:%S";

/// Register all date/time filters with the given environment.
pub fn register_filters(env: &mut Environment<'static>) {
    env.add_filter("strftime", strftime);
    env.add_filter("to_datetime", to_datetime);
}

/// Format an epoch second with a strftime pattern.
///
/// # Arguments
///
/// * `format` - The strftime pattern (the filter input, as in Ansible)
/// * `second` - Epoch second to format; defaults to the current time
/// * `utc` - Format in UTC instead of local time (default false)
fn strftime(format: String, second: Option<Value>, utc: Option<bool>) -> Result<String, Error> {
    let timestamp = match second {
        Some(value) if !value.is_none() && !value.is_undefined() => parse_epoch(&value)?,
        _ => Utc::now().timestamp(),
    };

    let datetime = DateTime::from_timestamp(timestamp, 0).ok_or_else(|| {
        Error::new(
            ErrorKind::InvalidOperation,
            format!("strftime: epoch second {} is out of range", timestamp),
        )
    })?;

    // chrono panics on an invalid specifier only when the result is consumed,
    // so format into a String and surface the failure as an error.
    let formatted = if utc.unwrap_or(false) {
        format_checked(&datetime, &format)
    } else {
        format_checked(&datetime.with_timezone(&Local), &format)
    };
    formatted.ok_or_else(|| {
        Error::new(
            ErrorKind::InvalidOperation,
            format!("strftime: invalid format string '{}'", format),
        )
    })
}

fn format_checked<Tz: TimeZone>(datetime: &DateTime<Tz>, format: &str) -> Option<String>
where
    Tz::Offset: std::fmt::Display,
{
    use std::fmt::Write;
    let mut out = String::new();
    write!(out, "{}", datetime.format(format)).ok()?;
    Some(out)
}

fn parse_epoch(value: &Value) -> Result<i64, Error> {
    if let Some(seconds) = value.as_i64() {
        return Ok(seconds);
    }
    value
        .to_string()
        .trim()
        .parse::<f64>()
        .map(|seconds| seconds as i64)
        .map_err(|_| {
            Error::new(
                ErrorKind::InvalidOperation,
                format!("strftime: '{}' is not an epoch second", value),
            )
        })
}

/// Parse a timestamp string into the normalized `%Y-%m-%d %H:%M:%S` form.
///
/// # Arguments
///
/// * `format` - strptime pattern of the input (default `%Y-%m-%d %H:%M:%S`)
fn to_datetime(value: Value, format: Option<String>) -> Result<String, Error> {
    let text = match value.as_str() {
        Some(text) => text.to_string(),
        None => value.to_string(),
    };
    let format = format.unwrap_or_else(|| DEFAULT_DATETIME_FORMAT.to_string());

    let parsed = NaiveDateTime::parse_from_str(text.trim(), &format)
        .or_else(|err| {
            // A date-only pattern parses into a NaiveDate; treat it as midnight.
            chrono::NaiveDate::parse_from_str(text.trim(), &format)
                .map(|date| date.and_hms_opt(0, 0, 0).expect("midnight is always valid"))
                .map_err(|_| err)
        })
        .map_err(|err| {
            Error::new(
                ErrorKind::InvalidOperation,
                format!(
                    "to_datetime: cannot parse '{}' with format '{}': {}",
                    text, format, err
                ),
            )
        })?;

    Ok(parsed.format(DEFAULT_DATETIME_FORMAT).to_string())
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

    fn render_err(template: &str) -> String {
        let mut env = Environment::new();
        register_filters(&mut env);
        env.template_from_str(template)
            .unwrap()
            .render(Value::UNDEFINED)
            .unwrap_err()
            .to_string()
    }

    #[test]
    fn test_strftime_with_epoch() {
        assert_eq!(
            render("{{ '%Y-%m-%d %H:%M:%S' | strftime(1666011400, true) }}"),
            "2022-10-17 12:56:40"
        );
        assert_eq!(render("{{ '%Y' | strftime(0, true) }}"), "1970");
    }

    #[test]
    fn test_strftime_defaults_to_now() {
        let year: i32 = render("{{ '%Y' | strftime(none, true) }}").parse().unwrap();
        assert!(year >= 2024, "formatted current year was {}", year);
    }

    #[test]
    fn test_strftime_rejects_non_numeric_epoch() {
        assert!(render_err("{{ '%Y' | strftime('yesterday') }}").contains("is not an epoch second"));
    }

    #[test]
    fn test_to_datetime() {
        assert_eq!(
            render("{{ '2019-12-12 12:00:00' | to_datetime }}"),
            "2019-12-12 12:00:00"
        );
        assert_eq!(
            render("{{ '12/12/2019' | to_datetime('%m/%d/%Y') }}"),
            "2019-12-12 00:00:00"
        );
    }

    #[test]
    fn test_to_datetime_reports_parse_failures() {
        assert!(render_err("{{ 'not-a-date' | to_datetime }}").contains("cannot parse"));
    }
}
