//! Jinja2 Filter Parity Test Suite for Issue #286
//!
//! These tests exercise the production template engine filters to ensure
//! expected Jinja2/Ansible-compatible behavior.

use rustible::template::TemplateEngine;
use serde_json::{json, Value as JsonValue};

fn render_expr(expr: &str, context: JsonValue) -> String {
    let engine = TemplateEngine::new();
    let template = format!("{{{{ {} }}}}", expr);
    engine
        .render_with_json(&template, &context)
        .expect("template rendering should succeed")
}

fn render_expr_json(expr: &str, context: JsonValue) -> JsonValue {
    let rendered = render_expr(&format!("{} | to_json", expr), context);
    serde_json::from_str(&rendered).expect("rendered output should be valid JSON")
}

// ============================================================================
// Tests: String Filters
// ============================================================================

#[test]
fn test_string_filters() {
    let cases = vec![
        (r#""HELLO"|lower"#, json!("hello")),
        (r#""hello"|upper"#, json!("HELLO")),
        (r#""hello world"|capitalize"#, json!("Hello world")),
        (r#""hello world"|title"#, json!("Hello World")),
        (r#""  hello  "|trim"#, json!("hello")),
        (
            r#""hello world"|replace("world", "rust")"#,
            json!("hello rust"),
        ),
    ];

    for (expr, expected) in cases {
        assert_eq!(render_expr_json(expr, json!({})), expected);
    }
}

#[test]
fn test_regex_filters() {
    assert_eq!(
        render_expr_json(r#""abc123"|regex_search("\\d+")"#, json!({})),
        json!("123")
    );
    assert_eq!(
        render_expr_json(r#""abc"|regex_search("\\d+")"#, json!({})),
        json!("")
    );
    assert_eq!(
        render_expr_json(r#""abc123"|regex_replace("\\d+", "x")"#, json!({})),
        json!("abcx")
    );
}

#[test]
fn test_split_join_filters() {
    assert_eq!(
        render_expr_json(r#""a,b,c"|split(",")"#, json!({})),
        json!(["a", "b", "c"])
    );
    assert_eq!(
        render_expr_json(r#"['a', 'b', 'c']|join(", ")"#, json!({})),
        json!("a, b, c")
    );
}

// ============================================================================
// Tests: Type Conversion Filters
// ============================================================================

#[test]
fn test_type_conversion_filters() {
    let cases = vec![
        (r#""42"|int"#, json!(42)),
        (r#""3.5"|float"#, json!(3.5)),
        (r#"1|string"#, json!("1")),
        (r#"''|bool"#, json!(false)),
        (r#""hello"|bool"#, json!(true)),
        (r#""ab"|list"#, json!(["a", "b"])),
    ];

    for (expr, expected) in cases {
        assert_eq!(render_expr_json(expr, json!({})), expected);
    }
}

// ============================================================================
// Tests: Collection Filters
// ============================================================================

#[test]
fn test_collection_filters() {
    let cases = vec![
        (r#""abc"|first"#, json!("a")),
        (r#""abc"|last"#, json!("c")),
        (r#""hello"|length"#, json!(5)),
        (r#""hello"|count"#, json!(5)),
        (r#"[1,2,1,3]|unique"#, json!([1, 2, 3])),
        (r#"[3,1,2]|sort"#, json!([1, 2, 3])),
        (r#"[1,2,3]|reverse"#, json!([3, 2, 1])),
        (r#"[[1,2],[3],4]|flatten"#, json!([1, 2, 3, 4])),
    ];

    for (expr, expected) in cases {
        assert_eq!(render_expr_json(expr, json!({})), expected);
    }
}

// ============================================================================
// Tests: Path Filters
// ============================================================================

#[test]
fn test_path_filters() {
    let cases = vec![
        (r#""/path/to/file.txt"|basename"#, json!("file.txt")),
        (r#""/path/to/file.txt"|dirname"#, json!("/path/to")),
        (r#""/tmp/file"|expanduser"#, json!("/tmp/file")),
        (r#""/does/not/exist"|realpath"#, json!("/does/not/exist")),
    ];

    for (expr, expected) in cases {
        assert_eq!(render_expr_json(expr, json!({})), expected);
    }
}

// ============================================================================
// Tests: Encoding & Serialization Filters
// ============================================================================

#[test]
fn test_encoding_filters() {
    assert_eq!(
        render_expr_json(r#""hello"|b64encode"#, json!({})),
        json!("aGVsbG8=")
    );
    assert_eq!(
        render_expr_json(r#""aGVsbG8="|b64decode"#, json!({})),
        json!("hello")
    );
}

#[test]
fn test_json_yaml_filters() {
    let json_rendered = render_expr(r#"{'a': 1, 'b': 2}|to_json"#, json!({}));
    let json_value: JsonValue =
        serde_json::from_str(&json_rendered).expect("to_json output should parse");
    assert_eq!(json_value, json!({"a": 1, "b": 2}));

    let context = json!({"payload": "{\"a\": 1}"});
    assert_eq!(
        render_expr_json("payload | from_json", context),
        json!({"a": 1})
    );

    assert_eq!(
        render_expr_json(r#"{'a': 1}|to_yaml|from_yaml"#, json!({})),
        json!({"a": 1})
    );

    let docs = json!({"docs": "---\na: 1\n---\na: 2\n"});
    assert_eq!(
        render_expr_json("docs | from_yaml_all", docs),
        json!([{"a": 1}, {"a": 2}])
    );

    assert_eq!(
        render_expr_json(r#"{'a': 1}|to_nice_yaml|from_yaml"#, json!({})),
        json!({"a": 1})
    );

    let pretty = render_expr(r#"{'a': 1}|to_nice_json"#, json!({}));
    let pretty_value: JsonValue =
        serde_json::from_str(&pretty).expect("to_nice_json output should parse");
    assert_eq!(pretty_value, json!({"a": 1}));
}

// ============================================================================
// Tests: Ansible-Specific Filters
// ============================================================================

#[test]
fn test_default_filter_and_alias() {
    assert_eq!(
        render_expr_json("missing | default('fallback')", json!({})),
        json!("fallback")
    );
    assert_eq!(
        render_expr_json("missing | d('fallback')", json!({})),
        json!("fallback")
    );
}

#[test]
fn test_mandatory_filter() {
    assert_eq!(
        render_expr_json("present | mandatory", json!({"present": "value"})),
        json!("value")
    );

    let engine = TemplateEngine::new();
    let template = "{{ missing | mandatory }}";
    assert!(engine.render_with_json(template, &json!({})).is_err());
}

#[test]
fn test_ternary_filter() {
    assert_eq!(
        render_expr_json(r#"true | ternary("yes", "no")"#, json!({})),
        json!("yes")
    );
    assert_eq!(
        render_expr_json(r#"false | ternary("yes", "no")"#, json!({})),
        json!("no")
    );
}

#[test]
fn test_combine_dict_filters() {
    assert_eq!(
        render_expr_json(r#"{'a': 1}|combine({'b': 2})"#, json!({})),
        json!({"a": 1, "b": 2})
    );

    assert_eq!(
        render_expr_json(r#"{'a': 1}|dict2items"#, json!({})),
        json!([{"key": "a", "value": 1}])
    );

    assert_eq!(
        render_expr_json(
            r#"[{'key': 'a', 'value': 1}, {'key': 'b', 'value': 2}]|items2dict"#,
            json!({})
        ),
        json!({"a": 1, "b": 2})
    );
}

#[test]
fn test_select_reject_map_filters() {
    let context = json!({
        "items": [
            {"name": "a", "enabled": true},
            {"name": "b", "enabled": false}
        ]
    });

    assert_eq!(
        render_expr_json("items | selectattr('enabled')", context.clone()),
        json!([{"name": "a", "enabled": true}])
    );
    assert_eq!(
        render_expr_json("items | rejectattr('enabled')", context.clone()),
        json!([{"name": "b", "enabled": false}])
    );
    // Jinja2 semantics: `map(attribute=...)` extracts a member, while
    // `map('filter')` applies a named filter to every element.
    assert_eq!(
        render_expr_json("items | map(attribute='name')", context.clone()),
        json!(["a", "b"])
    );
    assert_eq!(
        render_expr_json(
            "items | map(attribute='name') | map('upper')",
            context.clone()
        ),
        json!(["A", "B"])
    );
    assert_eq!(
        render_expr_json("items | selectattr('name', 'equalto', 'b')", context),
        json!([{"name": "b", "enabled": false}])
    );
}

// ============================================================================
// Tests: filter plugins reachable from the production engine
//
// These filters live in `src/plugins/filter` and are registered through
// `FilterRegistry`; the cases below pin the behavior playbooks see.
// ============================================================================

#[test]
fn test_regex_plugin_filters() {
    assert_eq!(
        render_expr_json(r#""a1b22c333"|regex_findall("\\d+")"#, json!({})),
        json!(["1", "22", "333"])
    );
    assert_eq!(
        render_expr_json(r#""a.b"|regex_escape"#, json!({})),
        json!("a\\.b")
    );
    assert_eq!(
        render_expr_json(r#""a1b2"|regex_split("\\d")"#, json!({})),
        json!(["a", "b", ""])
    );
}

#[test]
fn test_hash_filters() {
    assert_eq!(
        render_expr_json(r#""hello"|hash("md5")"#, json!({})),
        json!("5d41402abc4b2a76b9719d911017c592")
    );
    assert_eq!(
        render_expr_json(r#""hello"|checksum"#, json!({})),
        json!("aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d")
    );
    // crypt(3)-compatible output, verified against `openssl passwd -6`.
    assert_eq!(
        render_expr_json(
            r#""Hello world!"|password_hash("sha512", "saltstring")"#,
            json!({})
        ),
        json!("$6$saltstring$svn8UoSVapNtMuq1ukKS4tPQd8iKwSMHWjl/O817G3uBnIFNjnQJuesI68u4OTLiBFdcbYEdFCoEOfaS35inz1")
    );
}

#[test]
fn test_set_filters() {
    assert_eq!(
        render_expr_json("[1, 2, 3] | union([3, 4])", json!({})),
        json!([1, 2, 3, 4])
    );
    assert_eq!(
        render_expr_json("[1, 2, 3] | difference([3])", json!({})),
        json!([1, 2])
    );
    assert_eq!(
        render_expr_json("[1, 2, 3] | intersect([2, 3, 5])", json!({})),
        json!([2, 3])
    );
    assert_eq!(
        render_expr_json("[1, 2] | symmetric_difference([2, 3])", json!({})),
        json!([1, 3])
    );
}

#[test]
fn test_path_plugin_filters() {
    // Ansible's path_join takes the segments as a list.
    assert_eq!(
        render_expr_json(r#"["/etc", "nginx.conf"]|path_join"#, json!({})),
        json!("/etc/nginx.conf")
    );
    assert_eq!(
        render_expr_json(r#""/tmp/file.txt"|splitext"#, json!({})),
        json!(["/tmp/file", ".txt"])
    );
    assert_eq!(
        render_expr_json(r#""C:\\dir\\file.txt"|win_basename"#, json!({})),
        json!("file.txt")
    );
}

#[test]
fn test_math_plugin_filters() {
    assert_eq!(render_expr_json("8 | log(2)", json!({})), json!(3.0));
    assert_eq!(render_expr_json("2 | pow(10)", json!({})), json!(1024.0));
    assert_eq!(render_expr_json("8 | root(3)", json!({})), json!(2.0));
    assert_eq!(
        render_expr_json("1269730 | human_readable", json!({})),
        json!("1.21 MB")
    );
    assert_eq!(
        render_expr_json(r#""1.5 GB"|human_to_bytes"#, json!({})),
        json!(1610612736i64)
    );
}

#[test]
fn test_network_filters() {
    assert_eq!(
        render_expr_json(r#""192.168.0.5/24"|ipaddr("network")"#, json!({})),
        json!("192.168.0.0")
    );
    assert_eq!(
        render_expr_json(r#"["10.0.0.1", "bogus"]|ipaddr"#, json!({})),
        json!(["10.0.0.1"])
    );
    assert_eq!(
        render_expr_json(r#""10.0.0.0/8"|nthhost(305)"#, json!({})),
        json!("10.0.1.49")
    );
    assert_eq!(
        render_expr_json(r#""192.168.0.0/16"|ipsubnet(20, 1)"#, json!({})),
        json!("192.168.16.0/20")
    );
}

#[test]
fn test_string_plugin_filters() {
    assert_eq!(
        render_expr_json(r#""ab"|center(6)"#, json!({})),
        json!("  ab  ")
    );
    assert_eq!(
        render_expr_json(r#""hello wonderful world"|truncate(12)"#, json!({})),
        json!("hello...")
    );
    assert_eq!(
        render_expr_json(r#""notice"|comment"#, json!({})),
        json!("#\n# notice\n#")
    );
    assert_eq!(
        render_expr_json(r#""example"|to_uuid"#, json!({})),
        json!("0cd629ef-c3f7-5d62-98fc-b4270497b261")
    );
    assert_eq!(
        render_expr_json(r#"[1, 2]|type_debug"#, json!({})),
        json!("list")
    );
}

#[test]
fn test_datetime_filters() {
    assert_eq!(
        render_expr_json(r#""%Y-%m-%d"|strftime(1666011400, true)"#, json!({})),
        json!("2022-10-17")
    );
    assert_eq!(
        render_expr_json(r#""12/12/2019"|to_datetime("%m/%d/%Y")"#, json!({})),
        json!("2019-12-12 00:00:00")
    );
}

#[test]
fn test_collection_plugin_filters() {
    assert_eq!(
        render_expr_json("[1, 2, 3] | permutations(2) | length", json!({})),
        json!(6)
    );
    assert_eq!(
        render_expr_json("[1, 2, 3] | combinations(2) | length", json!({})),
        json!(3)
    );
    assert_eq!(
        render_expr_json("[0, 2] | map('extract', ['a', 'b', 'c']) | list", json!({})),
        json!(["a", "c"])
    );
    assert_eq!(
        render_expr_json(
            "[{'name': 'a', 'port': 80}] | rekey_on_member('name')",
            json!({})
        ),
        json!({"a": {"name": "a", "port": 80}})
    );
    assert_eq!(
        render_expr_json("[1, none, [2, none]] | flatten", json!({})),
        json!([1, 2])
    );
}

#[test]
fn test_encoding_plugin_filters() {
    assert_eq!(
        render_expr_json(r#""a b"|quote"#, json!({})),
        json!("'a b'")
    );
    assert_eq!(
        render_expr_json(
            r#""https://example.com/a?b=1"|urlsplit("hostname")"#,
            json!({})
        ),
        json!("example.com")
    );
}

#[test]
fn test_jinja_builtin_collection_filters_are_available() {
    // These come from MiniJinja rather than Rustible, but playbooks rely on
    // them, so a registration mistake must fail here.
    assert_eq!(
        render_expr_json("[3, 1, 2] | sort(reverse=true)", json!({})),
        json!([3, 2, 1])
    );
    assert_eq!(render_expr_json("[1, 2, 3] | sum", json!({})), json!(6));
    assert_eq!(render_expr_json("[1, 2, 3] | min", json!({})), json!(1));
    assert_eq!(render_expr_json("[1, 2, 3] | max", json!({})), json!(3));
    assert_eq!(
        render_expr_json("[1, 2, 3, 4] | batch(2) | length", json!({})),
        json!(2)
    );
    assert_eq!(
        render_expr_json("[1, 2, 3, 4] | slice(2) | length", json!({})),
        json!(2)
    );
    assert_eq!(
        render_expr_json("['a', 'b'] | zip([1, 2]) | list | length", json!({})),
        json!(2)
    );
    assert_eq!(
        render_expr_json("{'b': 2, 'a': 1} | dictsort | first", json!({})),
        json!(["a", 1])
    );
}

#[test]
fn test_default_filter_boolean_argument() {
    // `default(value, true)` also replaces falsy values, as in Ansible.
    assert_eq!(
        render_expr_json("'' | default('fallback', true)", json!({})),
        json!("fallback")
    );
    assert_eq!(
        render_expr_json("'set' | default('fallback', true)", json!({})),
        json!("set")
    );
    assert_eq!(
        render_expr_json("'' | default('fallback')", json!({})),
        json!("")
    );
}

// ============================================================================
// Tests: JMESPath, vault and the advanced ipaddr queries
//
// These three were the documented holes in filter parity. They are checked
// through the production engine, not the filter module, because a filter that
// is implemented but never registered is the failure these tests exist for.
// ============================================================================

#[test]
fn test_json_query_runs_in_the_production_engine() {
    let context = json!({
        "hosts": [
            {"name": "web1", "state": "up"},
            {"name": "web2", "state": "down"},
            {"name": "web3", "state": "up"},
        ]
    });
    assert_eq!(
        render_expr_json(r#"hosts | json_query("[?state=='up'].name")"#, context),
        json!(["web1", "web3"])
    );
}

#[test]
fn test_vault_round_trips_in_the_production_engine() {
    assert_eq!(
        render_expr(r#""s3cret" | vault("pw") | unvault("pw")"#, json!({})),
        "s3cret"
    );
}

#[test]
fn test_advanced_ipaddr_queries_are_registered() {
    let cases = vec![
        (
            r#""192.168.1.0/24" | ipaddr("range_usable")"#,
            "192.168.1.1-192.168.1.254",
        ),
        (r#""10.0.0.1/30" | ipaddr("peer")"#, "10.0.0.2"),
        (
            r#""192.168.1.5" | ipaddr("revdns")"#,
            "5.1.168.192.in-addr.arpa",
        ),
        (r#""192.168.1.5/24" | next_nth_usable(5)"#, "192.168.1.10"),
        (
            r#""1A:2B:3C:4D:5E:6F" | macaddr("cisco")"#,
            "1a2b.3c4d.5e6f",
        ),
    ];
    for (expr, expected) in cases {
        assert_eq!(render_expr(expr, json!({})), expected, "expr: {}", expr);
    }
}

#[test]
fn test_ipaddr_membership_filters_a_fact_list() {
    let context = json!({
        "ansible_all_ipv4_addresses": ["10.1.2.3", "192.168.1.5", "10.9.9.9"]
    });
    assert_eq!(
        render_expr_json(
            r#"ansible_all_ipv4_addresses | ipaddr("10.0.0.0/8")"#,
            context
        ),
        json!(["10.1.2.3", "10.9.9.9"])
    );
}
