//! Comprehensive tests for Jinja2 test plugins in Rustible
//!
//! This test suite verifies the Jinja2 test plugins (used with the `is` operator)
//! that are compatible with Ansible's Jinja2 tests.
//!
//! ## Test Categories
//!
//! ### Type Tests (15 tests)
//! - `is string`, `is number`, `is integer`, `is float`
//! - `is mapping`, `is iterable`, `is sequence`, `is boolean`
//! - `is none`, `is defined`, `is undefined`, `is callable`, `is sameas`
//!
//! ### String Tests (4 tests)
//! - `is lower`, `is upper`
//! - `is startswith`, `is endswith`
//!
//! ### Comparison Tests (12 tests)
//! - `is truthy`, `is falsy`, `is odd`, `is even`, `is divisibleby`
//! - `is version`, `is version_compare`
//! - `is gt`, `is ge`, `is lt`, `is le`, `is eq`, `is ne`
//! - `is positive`, `is negative`, `is zero`, `is between`
//!
//! ### Collection Tests (4 tests)
//! - `is contains`, `is empty`, `is in`
//!
//! ### File System Tests (7 tests)
//! - `is file`, `is directory`, `is dir`, `is link`, `is symlink`
//! - `is exists`, `is abs`
//!
//! ### Network Tests (6 tests)
//! - `is ip`, `is ipv4`, `is ipv6`, `is ipaddr`, `is mac`
//!
//! ### Pattern Tests (4 tests)
//! - `is match`, `is regex`
//!
//! ### Ansible-specific Tests (8 tests)
//! - `is url`, `is hostname`, `is uuid`
//! - `is success`, `is succeeded`, `is failed`, `is changed`, `is skipped`

use rustible::parser::Parser;
use indexmap::IndexMap;

// Helper function to create parser and render template
fn render(template: &str, vars: &IndexMap<String, serde_yaml::Value>) -> String {
    let parser = Parser::new();
    parser.render_template(template, vars).unwrap()
}

// Helper function to check if template evaluates to true
fn eval_test(template: &str, vars: &IndexMap<String, serde_yaml::Value>) -> bool {
    let result = render(template, vars);
    result.trim().to_lowercase() == "true" || result.trim() == "yes" || result.trim() == "1"
}

// ============================================================================
// Type Tests
// ============================================================================

#[test]
fn test_is_string() {
    let mut vars = IndexMap::new();
    vars.insert("str_val".to_string(), serde_yaml::Value::String("hello".to_string()));
    vars.insert("int_val".to_string(), serde_yaml::Value::Number(42.into()));

    let template = "{% if str_val is string %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "true");

    let template = "{% if int_val is string %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "false");
}

#[test]
fn test_is_number() {
    let mut vars = IndexMap::new();
    vars.insert("int_val".to_string(), serde_yaml::Value::Number(42.into()));
    vars.insert("float_val".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(3.14)));
    vars.insert("str_val".to_string(), serde_yaml::Value::String("hello".to_string()));

    let template = "{% if int_val is number %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "true");

    let template = "{% if float_val is number %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "true");

    let template = "{% if str_val is number %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "false");
}

#[test]
fn test_is_integer() {
    let mut vars = IndexMap::new();
    vars.insert("int_val".to_string(), serde_yaml::Value::Number(42.into()));
    vars.insert("str_val".to_string(), serde_yaml::Value::String("hello".to_string()));

    let template = "{% if int_val is integer %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "true");

    let template = "{% if str_val is integer %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "false");
}

#[test]
fn test_is_mapping() {
    let mut vars = IndexMap::new();
    let mut mapping = serde_yaml::Mapping::new();
    mapping.insert(serde_yaml::Value::String("key".to_string()), serde_yaml::Value::String("value".to_string()));
    vars.insert("dict_val".to_string(), serde_yaml::Value::Mapping(mapping));
    vars.insert("str_val".to_string(), serde_yaml::Value::String("hello".to_string()));

    let template = "{% if dict_val is mapping %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "true");

    let template = "{% if str_val is mapping %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "false");
}

#[test]
fn test_is_iterable() {
    let mut vars = IndexMap::new();
    vars.insert("list_val".to_string(), serde_yaml::Value::Sequence(vec![
        serde_yaml::Value::Number(1.into()),
        serde_yaml::Value::Number(2.into()),
    ]));
    vars.insert("str_val".to_string(), serde_yaml::Value::String("hello".to_string()));
    vars.insert("int_val".to_string(), serde_yaml::Value::Number(42.into()));

    let template = "{% if list_val is iterable %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "true");

    // Strings are iterable
    let template = "{% if str_val is iterable %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "true");
}

#[test]
fn test_is_sequence() {
    let mut vars = IndexMap::new();
    vars.insert("list_val".to_string(), serde_yaml::Value::Sequence(vec![
        serde_yaml::Value::Number(1.into()),
        serde_yaml::Value::Number(2.into()),
    ]));
    vars.insert("str_val".to_string(), serde_yaml::Value::String("hello".to_string()));

    let template = "{% if list_val is sequence %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "true");

    let template = "{% if str_val is sequence %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "false");
}

#[test]
fn test_is_boolean() {
    let mut vars = IndexMap::new();
    vars.insert("bool_val".to_string(), serde_yaml::Value::Bool(true));
    vars.insert("str_val".to_string(), serde_yaml::Value::String("hello".to_string()));

    let template = "{% if bool_val is boolean %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "true");

    let template = "{% if str_val is boolean %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "false");
}

#[test]
fn test_is_none() {
    let mut vars = IndexMap::new();
    vars.insert("none_val".to_string(), serde_yaml::Value::Null);
    vars.insert("str_val".to_string(), serde_yaml::Value::String("hello".to_string()));

    let template = "{% if none_val is none %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "true");

    let template = "{% if str_val is none %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "false");
}

#[test]
fn test_is_defined() {
    let mut vars = IndexMap::new();
    vars.insert("defined_var".to_string(), serde_yaml::Value::String("value".to_string()));

    let template = "{% if defined_var is defined %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "true");

    let template = "{% if undefined_var is defined %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "false");
}

#[test]
fn test_is_undefined() {
    let mut vars = IndexMap::new();
    vars.insert("defined_var".to_string(), serde_yaml::Value::String("value".to_string()));

    let template = "{% if undefined_var is undefined %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "true");

    let template = "{% if defined_var is undefined %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "false");
}

#[test]
fn test_is_sameas() {
    let mut vars = IndexMap::new();
    vars.insert("val1".to_string(), serde_yaml::Value::String("hello".to_string()));
    vars.insert("val2".to_string(), serde_yaml::Value::String("hello".to_string()));
    vars.insert("val3".to_string(), serde_yaml::Value::String("world".to_string()));

    let template = "{% if val1 is sameas(val2) %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "true");

    let template = "{% if val1 is sameas(val3) %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "false");
}

// ============================================================================
// String Tests
// ============================================================================

#[test]
fn test_is_lower() {
    let mut vars = IndexMap::new();
    vars.insert("lower_str".to_string(), serde_yaml::Value::String("hello".to_string()));
    vars.insert("upper_str".to_string(), serde_yaml::Value::String("HELLO".to_string()));

    let template = "{% if lower_str is lower %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "true");

    let template = "{% if upper_str is lower %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "false");
}

#[test]
fn test_is_upper() {
    let mut vars = IndexMap::new();
    vars.insert("upper_str".to_string(), serde_yaml::Value::String("HELLO".to_string()));
    vars.insert("lower_str".to_string(), serde_yaml::Value::String("hello".to_string()));

    let template = "{% if upper_str is upper %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "true");

    let template = "{% if lower_str is upper %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "false");
}

#[test]
fn test_is_startswith() {
    let mut vars = IndexMap::new();
    vars.insert("text".to_string(), serde_yaml::Value::String("hello world".to_string()));

    let template = "{% if text is startswith('hello') %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "true");

    let template = "{% if text is startswith('world') %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "false");
}

#[test]
fn test_is_endswith() {
    let mut vars = IndexMap::new();
    vars.insert("text".to_string(), serde_yaml::Value::String("hello world".to_string()));

    let template = "{% if text is endswith('world') %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "true");

    let template = "{% if text is endswith('hello') %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "false");
}

// ============================================================================
// Comparison Tests
// ============================================================================

#[test]
fn test_is_truthy() {
    let mut vars = IndexMap::new();
    vars.insert("true_val".to_string(), serde_yaml::Value::Bool(true));
    vars.insert("one_val".to_string(), serde_yaml::Value::Number(1.into()));
    vars.insert("zero_val".to_string(), serde_yaml::Value::Number(0.into()));
    vars.insert("str_val".to_string(), serde_yaml::Value::String("hello".to_string()));

    let template = "{% if true_val is truthy %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "true");

    let template = "{% if one_val is truthy %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "true");

    let template = "{% if str_val is truthy %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "true");
}

#[test]
fn test_is_falsy() {
    let mut vars = IndexMap::new();
    vars.insert("false_val".to_string(), serde_yaml::Value::Bool(false));
    vars.insert("zero_val".to_string(), serde_yaml::Value::Number(0.into()));
    vars.insert("empty_str".to_string(), serde_yaml::Value::String("".to_string()));

    let template = "{% if false_val is falsy %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "true");

    let template = "{% if zero_val is falsy %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "true");
}

#[test]
fn test_is_odd() {
    let mut vars = IndexMap::new();
    vars.insert("odd_num".to_string(), serde_yaml::Value::Number(7.into()));
    vars.insert("even_num".to_string(), serde_yaml::Value::Number(8.into()));

    let template = "{% if odd_num is odd %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "true");

    let template = "{% if even_num is odd %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "false");
}

#[test]
fn test_is_even() {
    let mut vars = IndexMap::new();
    vars.insert("even_num".to_string(), serde_yaml::Value::Number(8.into()));
    vars.insert("odd_num".to_string(), serde_yaml::Value::Number(7.into()));

    let template = "{% if even_num is even %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "true");

    let template = "{% if odd_num is even %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "false");
}

#[test]
fn test_is_divisibleby() {
    let mut vars = IndexMap::new();
    vars.insert("num".to_string(), serde_yaml::Value::Number(12.into()));

    let template = "{% if num is divisibleby(3) %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "true");

    let template = "{% if num is divisibleby(5) %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "false");
}

#[test]
fn test_is_version() {
    let mut vars = IndexMap::new();
    vars.insert("ver".to_string(), serde_yaml::Value::String("2.1.0".to_string()));

    let template = "{% if ver is version('2.0.0', '>=') %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "true");

    let template = "{% if ver is version('3.0.0', '>=') %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "false");

    let template = "{% if ver is version('2.1.0', '==') %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "true");
}

#[test]
fn test_is_version_compare() {
    let mut vars = IndexMap::new();
    vars.insert("ver".to_string(), serde_yaml::Value::String("1.5.0".to_string()));

    let template = "{% if ver is version_compare('1.0.0', '>') %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "true");

    let template = "{% if ver is version_compare('2.0.0', '<') %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "true");
}

#[test]
fn test_is_positive() {
    let mut vars = IndexMap::new();
    vars.insert("pos_num".to_string(), serde_yaml::Value::Number(5.into()));
    vars.insert("neg_num".to_string(), serde_yaml::Value::Number((-5).into()));
    vars.insert("zero".to_string(), serde_yaml::Value::Number(0.into()));

    let template = "{% if pos_num is positive %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "true");

    let template = "{% if neg_num is positive %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "false");

    let template = "{% if zero is positive %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "false");
}

#[test]
fn test_is_negative() {
    let mut vars = IndexMap::new();
    vars.insert("neg_num".to_string(), serde_yaml::Value::Number((-5).into()));
    vars.insert("pos_num".to_string(), serde_yaml::Value::Number(5.into()));

    let template = "{% if neg_num is negative %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "true");

    let template = "{% if pos_num is negative %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "false");
}

#[test]
fn test_is_zero() {
    let mut vars = IndexMap::new();
    vars.insert("zero".to_string(), serde_yaml::Value::Number(0.into()));
    vars.insert("one".to_string(), serde_yaml::Value::Number(1.into()));

    let template = "{% if zero is zero %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "true");

    let template = "{% if one is zero %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "false");
}

#[test]
fn test_is_between() {
    let mut vars = IndexMap::new();
    vars.insert("num".to_string(), serde_yaml::Value::Number(5.into()));

    let template = "{% if num is between(1, 10) %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "true");

    let template = "{% if num is between(6, 10) %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "false");
}

#[test]
fn test_is_gt_ge_lt_le() {
    let mut vars = IndexMap::new();
    vars.insert("num".to_string(), serde_yaml::Value::Number(5.into()));

    let template = "{% if num is gt(3) %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "true");

    let template = "{% if num is ge(5) %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "true");

    let template = "{% if num is lt(10) %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "true");

    let template = "{% if num is le(5) %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "true");

    let template = "{% if num is gt(5) %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "false");
}

#[test]
fn test_is_eq_ne() {
    let mut vars = IndexMap::new();
    vars.insert("val1".to_string(), serde_yaml::Value::String("hello".to_string()));
    vars.insert("val2".to_string(), serde_yaml::Value::String("hello".to_string()));
    vars.insert("val3".to_string(), serde_yaml::Value::String("world".to_string()));

    let template = "{% if val1 is eq(val2) %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "true");

    let template = "{% if val1 is ne(val3) %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "true");
}

// ============================================================================
// Collection Tests
// ============================================================================

#[test]
fn test_is_contains() {
    let mut vars = IndexMap::new();
    vars.insert("list".to_string(), serde_yaml::Value::Sequence(vec![
        serde_yaml::Value::String("a".to_string()),
        serde_yaml::Value::String("b".to_string()),
        serde_yaml::Value::String("c".to_string()),
    ]));
    vars.insert("item".to_string(), serde_yaml::Value::String("b".to_string()));
    vars.insert("missing".to_string(), serde_yaml::Value::String("z".to_string()));

    let template = "{% if list is contains(item) %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "true");

    let template = "{% if list is contains(missing) %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "false");
}

#[test]
fn test_is_empty() {
    let mut vars = IndexMap::new();
    vars.insert("empty_list".to_string(), serde_yaml::Value::Sequence(vec![]));
    vars.insert("non_empty_list".to_string(), serde_yaml::Value::Sequence(vec![
        serde_yaml::Value::Number(1.into()),
    ]));
    vars.insert("empty_str".to_string(), serde_yaml::Value::String("".to_string()));
    vars.insert("non_empty_str".to_string(), serde_yaml::Value::String("hello".to_string()));

    let template = "{% if empty_list is empty %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "true");

    let template = "{% if non_empty_list is empty %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "false");

    let template = "{% if empty_str is empty %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "true");
}

#[test]
fn test_is_in() {
    let mut vars = IndexMap::new();
    vars.insert("item".to_string(), serde_yaml::Value::String("b".to_string()));
    vars.insert("list".to_string(), serde_yaml::Value::Sequence(vec![
        serde_yaml::Value::String("a".to_string()),
        serde_yaml::Value::String("b".to_string()),
        serde_yaml::Value::String("c".to_string()),
    ]));

    let template = "{% if item is in(list) %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "true");
}

// ============================================================================
// File System Tests
// ============================================================================

#[test]
fn test_is_file() {
    let mut vars = IndexMap::new();
    // Test with a file that likely exists on any system
    vars.insert("real_file".to_string(), serde_yaml::Value::String("/etc/passwd".to_string()));
    vars.insert("fake_file".to_string(), serde_yaml::Value::String("/nonexistent/file.txt".to_string()));

    // This test may need adjustment based on the system
    #[cfg(unix)]
    {
        let template = "{% if real_file is file %}true{% else %}false{% endif %}";
        assert_eq!(render(template, &vars), "true");
    }

    let template = "{% if fake_file is file %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "false");
}

#[test]
fn test_is_directory() {
    let mut vars = IndexMap::new();
    vars.insert("real_dir".to_string(), serde_yaml::Value::String("/tmp".to_string()));
    vars.insert("fake_dir".to_string(), serde_yaml::Value::String("/nonexistent/directory".to_string()));

    #[cfg(unix)]
    {
        let template = "{% if real_dir is directory %}true{% else %}false{% endif %}";
        assert_eq!(render(template, &vars), "true");
    }

    let template = "{% if fake_dir is directory %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "false");
}

#[test]
fn test_is_exists() {
    let mut vars = IndexMap::new();
    vars.insert("real_path".to_string(), serde_yaml::Value::String("/tmp".to_string()));
    vars.insert("fake_path".to_string(), serde_yaml::Value::String("/nonexistent/path".to_string()));

    #[cfg(unix)]
    {
        let template = "{% if real_path is exists %}true{% else %}false{% endif %}";
        assert_eq!(render(template, &vars), "true");
    }

    let template = "{% if fake_path is exists %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "false");
}

#[test]
fn test_is_abs() {
    let mut vars = IndexMap::new();
    vars.insert("abs_path".to_string(), serde_yaml::Value::String("/usr/bin".to_string()));
    vars.insert("rel_path".to_string(), serde_yaml::Value::String("relative/path".to_string()));

    let template = "{% if abs_path is abs %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "true");

    let template = "{% if rel_path is abs %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "false");
}

// ============================================================================
// Network Tests
// ============================================================================

#[test]
fn test_is_ip() {
    let mut vars = IndexMap::new();
    vars.insert("ipv4".to_string(), serde_yaml::Value::String("192.168.1.1".to_string()));
    vars.insert("ipv6".to_string(), serde_yaml::Value::String("::1".to_string()));
    vars.insert("not_ip".to_string(), serde_yaml::Value::String("not.an.ip.address".to_string()));

    let template = "{% if ipv4 is ip %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "true");

    let template = "{% if ipv6 is ip %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "true");

    let template = "{% if not_ip is ip %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "false");
}

#[test]
fn test_is_ipv4() {
    let mut vars = IndexMap::new();
    vars.insert("valid_ipv4".to_string(), serde_yaml::Value::String("192.168.1.1".to_string()));
    vars.insert("ipv6".to_string(), serde_yaml::Value::String("::1".to_string()));
    vars.insert("invalid".to_string(), serde_yaml::Value::String("256.256.256.256".to_string()));

    let template = "{% if valid_ipv4 is ipv4 %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "true");

    let template = "{% if ipv6 is ipv4 %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "false");

    let template = "{% if invalid is ipv4 %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "false");
}

#[test]
fn test_is_ipv6() {
    let mut vars = IndexMap::new();
    vars.insert("valid_ipv6".to_string(), serde_yaml::Value::String("2001:db8::1".to_string()));
    vars.insert("localhost_ipv6".to_string(), serde_yaml::Value::String("::1".to_string()));
    vars.insert("ipv4".to_string(), serde_yaml::Value::String("192.168.1.1".to_string()));

    let template = "{% if valid_ipv6 is ipv6 %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "true");

    let template = "{% if localhost_ipv6 is ipv6 %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "true");

    let template = "{% if ipv4 is ipv6 %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "false");
}

#[test]
fn test_is_ipaddr() {
    let mut vars = IndexMap::new();
    vars.insert("cidr".to_string(), serde_yaml::Value::String("192.168.1.0/24".to_string()));
    vars.insert("plain_ip".to_string(), serde_yaml::Value::String("192.168.1.1".to_string()));
    vars.insert("invalid".to_string(), serde_yaml::Value::String("192.168.1.0/99".to_string()));

    let template = "{% if cidr is ipaddr %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "true");

    let template = "{% if plain_ip is ipaddr %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "true");

    let template = "{% if invalid is ipaddr %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "false");
}

#[test]
fn test_is_mac() {
    let mut vars = IndexMap::new();
    vars.insert("mac_colon".to_string(), serde_yaml::Value::String("00:11:22:33:44:55".to_string()));
    vars.insert("mac_dash".to_string(), serde_yaml::Value::String("00-11-22-33-44-55".to_string()));
    vars.insert("mac_dot".to_string(), serde_yaml::Value::String("0011.2233.4455".to_string()));
    vars.insert("invalid_mac".to_string(), serde_yaml::Value::String("00:11:22:33".to_string()));

    let template = "{% if mac_colon is mac %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "true");

    let template = "{% if mac_dash is mac %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "true");

    let template = "{% if mac_dot is mac %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "true");

    let template = "{% if invalid_mac is mac %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "false");
}

// ============================================================================
// Pattern Tests
// ============================================================================

#[test]
fn test_is_match() {
    let mut vars = IndexMap::new();
    vars.insert("text".to_string(), serde_yaml::Value::String("hello123world".to_string()));

    let template = r#"{% if text is match('[0-9]+') %}true{% else %}false{% endif %}"#;
    assert_eq!(render(template, &vars), "true");

    let template = r#"{% if text is match('^hello') %}true{% else %}false{% endif %}"#;
    assert_eq!(render(template, &vars), "true");

    let template = r#"{% if text is match('^world') %}true{% else %}false{% endif %}"#;
    assert_eq!(render(template, &vars), "false");
}

#[test]
fn test_is_regex() {
    let mut vars = IndexMap::new();
    vars.insert("email".to_string(), serde_yaml::Value::String("test@example.com".to_string()));
    vars.insert("not_email".to_string(), serde_yaml::Value::String("not-an-email".to_string()));

    // Simple email-like pattern (not comprehensive)
    let template = r#"{% if email is regex('.+@.+\..+') %}true{% else %}false{% endif %}"#;
    assert_eq!(render(template, &vars), "true");

    let template = r#"{% if not_email is regex('.+@.+\..+') %}true{% else %}false{% endif %}"#;
    assert_eq!(render(template, &vars), "false");
}

// ============================================================================
// Ansible-specific Tests
// ============================================================================

#[test]
fn test_is_url() {
    let mut vars = IndexMap::new();
    vars.insert("valid_url".to_string(), serde_yaml::Value::String("https://example.com/path".to_string()));
    vars.insert("invalid_url".to_string(), serde_yaml::Value::String("not-a-url".to_string()));

    let template = "{% if valid_url is url %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "true");

    let template = "{% if invalid_url is url %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "false");
}

#[test]
fn test_is_hostname() {
    let mut vars = IndexMap::new();
    vars.insert("valid_host".to_string(), serde_yaml::Value::String("example.com".to_string()));
    vars.insert("also_valid".to_string(), serde_yaml::Value::String("server-01.internal.company.com".to_string()));
    vars.insert("invalid_host".to_string(), serde_yaml::Value::String("-invalid".to_string()));

    let template = "{% if valid_host is hostname %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "true");

    let template = "{% if also_valid is hostname %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "true");

    let template = "{% if invalid_host is hostname %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "false");
}

#[test]
fn test_is_uuid() {
    let mut vars = IndexMap::new();
    vars.insert("valid_uuid".to_string(), serde_yaml::Value::String("550e8400-e29b-41d4-a716-446655440000".to_string()));
    vars.insert("invalid_uuid".to_string(), serde_yaml::Value::String("not-a-uuid".to_string()));

    let template = "{% if valid_uuid is uuid %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "true");

    let template = "{% if invalid_uuid is uuid %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "false");
}

#[test]
fn test_is_success_and_failed() {
    let mut vars = IndexMap::new();

    let mut success_result = serde_yaml::Mapping::new();
    success_result.insert(serde_yaml::Value::String("failed".to_string()), serde_yaml::Value::Bool(false));
    success_result.insert(serde_yaml::Value::String("rc".to_string()), serde_yaml::Value::Number(0.into()));
    vars.insert("success_result".to_string(), serde_yaml::Value::Mapping(success_result));

    let mut failed_result = serde_yaml::Mapping::new();
    failed_result.insert(serde_yaml::Value::String("failed".to_string()), serde_yaml::Value::Bool(true));
    failed_result.insert(serde_yaml::Value::String("rc".to_string()), serde_yaml::Value::Number(1.into()));
    vars.insert("failed_result".to_string(), serde_yaml::Value::Mapping(failed_result));

    let template = "{% if success_result is success %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "true");

    let template = "{% if success_result is succeeded %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "true");

    let template = "{% if failed_result is failed %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "true");

    let template = "{% if success_result is failed %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "false");
}

#[test]
fn test_is_changed() {
    let mut vars = IndexMap::new();

    let mut changed_result = serde_yaml::Mapping::new();
    changed_result.insert(serde_yaml::Value::String("changed".to_string()), serde_yaml::Value::Bool(true));
    vars.insert("changed_result".to_string(), serde_yaml::Value::Mapping(changed_result));

    let mut unchanged_result = serde_yaml::Mapping::new();
    unchanged_result.insert(serde_yaml::Value::String("changed".to_string()), serde_yaml::Value::Bool(false));
    vars.insert("unchanged_result".to_string(), serde_yaml::Value::Mapping(unchanged_result));

    let template = "{% if changed_result is changed %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "true");

    let template = "{% if unchanged_result is changed %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "false");
}

#[test]
fn test_is_skipped() {
    let mut vars = IndexMap::new();

    let mut skipped_result = serde_yaml::Mapping::new();
    skipped_result.insert(serde_yaml::Value::String("skipped".to_string()), serde_yaml::Value::Bool(true));
    vars.insert("skipped_result".to_string(), serde_yaml::Value::Mapping(skipped_result));

    let mut not_skipped = serde_yaml::Mapping::new();
    not_skipped.insert(serde_yaml::Value::String("skipped".to_string()), serde_yaml::Value::Bool(false));
    vars.insert("not_skipped".to_string(), serde_yaml::Value::Mapping(not_skipped));

    let template = "{% if skipped_result is skipped %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "true");

    let template = "{% if not_skipped is skipped %}true{% else %}false{% endif %}";
    assert_eq!(render(template, &vars), "false");
}
