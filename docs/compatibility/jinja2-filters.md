# Jinja2 Filter Compatibility

> **Last Updated:** 2026-09-10
> **Rustible Version:** 0.1.x

This document tracks the compatibility between Ansible's Jinja2 filters and
Rustible's MiniJinja-based template engine.

Every filter listed as available is exercised from the production engine in
`tests/jinja2_filter_parity_tests.rs`. Filters come from three places:

- MiniJinja's Jinja2 built-ins (`min`, `max`, `sum`, `batch`, `slice`,
  `groupby`, `zip`, `select`, `reject`, `selectattr`, `rejectattr`, `map`,
  `dictsort`, `indent`, ...).
- Rustible's filter plugins in `src/plugins/filter`, registered through
  `FilterRegistry::register_all`.
- Engine-local filters in `src/template.rs`, which override a name only when
  Ansible's semantics differ from Jinja2's (for example `bool`).

---

## Summary

| Category | Available | Notes |
|----------|-----------|-------|
| String | Yes | Jinja2 set plus `comment`, `to_uuid`, `quote` |
| List / set | Yes | Ansible set operations plus Jinja2 built-ins |
| Dict | Yes | `combine`, `dict2items`, `items2dict`, `rekey_on_member`, `dictsort` |
| Math | Yes | `log`, `pow`, `root`, `human_readable`, `human_to_bytes` |
| Type conversion | Yes | Ansible truthy semantics for `bool` |
| Path | Yes | POSIX and Windows paths |
| Encoding / hashing | Yes | Includes crypt(3)-compatible `password_hash` |
| Network (`ipaddr` family) | Partial | Common queries; see the gaps section |
| Date / time | Partial | `strftime`, `to_datetime` (returns a string) |
| JMESPath (`json_query`) | No | Not implemented |

---

## Available Filters

### String

| Filter | Source | Notes |
|--------|--------|-------|
| `default` / `d` | Rustible | Supports `default(value, true)` and `value=` |
| `lower`, `upper`, `capitalize`, `title`, `trim` | Rustible | |
| `replace` | Rustible | |
| `split`, `join` | Rustible | |
| `regex_replace`, `regex_search` | Rustible | |
| `regex_findall`, `regex_escape`, `regex_split`, `regex_match` | Plugin | |
| `center`, `truncate`, `wordwrap`, `wordcount` | Plugin | Jinja2 semantics, including `truncate`'s leeway |
| `indent`, `lines`, `format`, `pprint` | MiniJinja | |
| `comment` | Plugin | Styles `plain`, `c`, `cblock`, `erlang`, `xml` |
| `quote`, `unquote` | Plugin | Shell quoting |
| `to_uuid` | Plugin | UUIDv5 in Ansible's namespace |
| `type_debug` | Plugin | Python-style type names |
| `mandatory` | Rustible | Fails when undefined |
| `ternary` | Rustible | |

### List and set

| Filter | Source | Notes |
|--------|--------|-------|
| `first`, `last`, `length`, `count`, `reverse`, `sort` | MiniJinja | `sort` supports `attribute=`, `reverse=`, `case_sensitive=` |
| `min`, `max`, `sum`, `batch`, `slice`, `groupby`, `zip`, `chain` | MiniJinja | |
| `select`, `reject`, `selectattr`, `rejectattr`, `map` | MiniJinja | Every registered test works; `map('filter')` and `map(attribute=)` both supported |
| `unique` | Plugin | Case-sensitive by default; `attribute=`, `case_sensitive=` |
| `flatten` | Plugin | `levels`, `skip_nulls` (nulls dropped by default, as in Ansible) |
| `union`, `difference`, `intersect`, `symmetric_difference` | Plugin | |
| `product`, `permutations`, `combinations` | Plugin | |
| `zip_longest`, `subelements` | Plugin | |
| `extract` | Plugin | Pairs with `map('extract', container)` |
| `random`, `shuffle` | Plugin | `seed=` is reproducible, but not Python's sequence |
| `list` | Rustible | Also converts lazy iterables and strings |

### Dictionary

| Filter | Source | Notes |
|--------|--------|-------|
| `combine` | Rustible | Recursive merge |
| `dict2items`, `items2dict` | Rustible | |
| `rekey_on_member` | Plugin | `duplicates='error'` (default) or `'overwrite'` |
| `dictsort`, `items`, `attr` | MiniJinja | |

### Math

| Filter | Source | Notes |
|--------|--------|-------|
| `int`, `float`, `abs`, `round` | Rustible / MiniJinja | |
| `log`, `pow`, `root` | Plugin | |
| `human_readable` | Plugin | `human_readable(isbits, unit)`; `precision=` is a Rustible extension |
| `human_to_bytes` | Plugin | 1024-based, accepts `default_unit` and `isbits` |

### Type conversion

| Filter | Source | Notes |
|--------|--------|-------|
| `bool` | Rustible | Ansible truthy strings (`yes`, `on`, `1`, ...) |
| `string`, `int`, `float`, `list` | Rustible | |
| `type_debug` | Plugin | |

### Path

| Filter | Source | Notes |
|--------|--------|-------|
| `basename`, `dirname`, `expanduser`, `realpath` | Rustible | |
| `path_join` | Plugin | Takes the segments as a list, as in Ansible |
| `splitext`, `relpath`, `expandvars` | Plugin | |
| `win_basename`, `win_dirname`, `win_splitdrive` | Plugin | |

### Encoding, serialization and hashing

| Filter | Source | Notes |
|--------|--------|-------|
| `b64encode`, `b64decode` | Rustible | |
| `to_json`, `to_nice_json`, `from_json` | Rustible | |
| `to_yaml`, `to_nice_yaml`, `from_yaml`, `from_yaml_all` | Rustible | |
| `urlencode`, `urldecode`, `urlsplit` | Plugin | |
| `hash` | Plugin | md5, sha1 (default), sha256, sha384, sha512; unknown algorithms are an error |
| `checksum` | Plugin | SHA-1, as in Ansible |
| `md5`, `sha1`, `sha256`, `sha512` | Plugin | |
| `password_hash` | Plugin | Real SHA-crypt: `$6$` (default) and `$5$`, `rounds=` and explicit salts supported |

### Network

| Filter | Source | Notes |
|--------|--------|-------|
| `ipaddr` | Plugin | Validation, list filtering, integer index, and the queries below |
| `ipv4`, `ipv6` | Plugin | Family filtering with the same queries |
| `ipsubnet` | Plugin | Subnet count and nth subnet |
| `ipmath`, `nthhost` | Plugin | |
| `network_in_usable` | Plugin | |
| `cidr_merge` | Plugin | `merge` (default) and `span` |
| `ipwrap` | Plugin | Brackets IPv6 addresses |

`ipaddr` queries: `address`, `ip`, `address/prefix`, `host`, `prefix`,
`netmask`, `hostmask`, `wildcard`, `network`, `broadcast`, `net`, `subnet`,
`size`, `first_usable`, `last_usable`, `version`, `4`/`ipv4`, `6`/`ipv6`,
`public`, `private`, `loopback`, `multicast`, `link-local`, `unspecified`.
An integer query selects the nth address in the network. Unsupported queries
raise an error rather than returning a wrong answer.

### Date and time

| Filter | Source | Notes |
|--------|--------|-------|
| `strftime` | Plugin | Format string is the input; optional epoch second and `utc` flag |
| `to_datetime` | Plugin | Returns a normalized `%Y-%m-%d %H:%M:%S` string |

---

## Known Gaps

| Filter | Status | Notes |
|--------|--------|-------|
| `json_query` | Not implemented | Requires a JMESPath engine; no dependency added yet |
| `vault`, `unvault` | Not implemented | Vault operations are available through the `vault` CLI and lookups |
| `ipaddr` advanced queries | Not implemented | `next_usable`, `previous_usable`, `range_usable`, `peer`, `6to4`, `teredo` |
| `password_hash` schemes | Partial | Only `sha512` and `sha256` crypt; `bcrypt`, `md5_crypt` and `des_crypt` raise an error |
| `hash` algorithms | Partial | No `blake2b`/`blake2s` |
| `to_datetime` return type | Different | Ansible returns a `datetime` object supporting arithmetic; Rustible returns a string, so date arithmetic must go through `strftime` and epoch seconds |
| `random` / `shuffle` seeding | Different | Deterministic per seed, but not Python's `random` sequence |
| Collection filters | Out of scope | `k8s_config_resource_name`, `parse_cli`, `parse_xml` and other collection-provided filters |

---

## Jinja2 Tests

All Ansible-documented tests are available. Rustible registers `defined`,
`undefined`, `none`/`null`, `truthy`, `falsy`, `boolean`, `integer`, `float`,
`number`, `string`, `mapping`/`dict`, `iterable`, `sequence`/`list`, `sameas`,
`contains`, `match`, `search`, `startswith`, `endswith`, `file`, `directory`,
`link`, `exists`, `abs`, `success`, `failed`, `changed`, `skipped`, `odd`,
`even`, `divisibleby`, `in`, `subset`, `superset`, `callable` and `escaped`.

MiniJinja adds the comparison tests `eq`/`equalto`/`==`, `ne`/`!=`,
`lt`/`lessthan`/`<`, `le`/`<=`, `gt`/`greaterthan`/`>`, `ge`/`>=`, plus
`startingwith`, `endingwith`, `lower`, `upper` and `safe`.

---

## Known Differences

### 1. Boolean handling

Ansible accepts various truthy strings (`yes`, `no`, `true`, `false`, `on`,
`off`). Rustible's `bool` filter handles these:

```yaml
- when: "{{ 'yes' | bool }}"
- when: "{{ enable_feature | bool }}"
```

### 2. Undefined variable behavior

Rustible uses MiniJinja's `Chainable` undefined behavior, matching Ansible's
default:

```yaml
- debug: msg="{{ undefined_var }}"
```

### 3. `default` and null

`{{ null_var | default('x') }}` returns `x` in Rustible. Jinja2 replaces only
undefined values, so Ansible would return the null. Pass `default('x', true)`
when you want falsy values replaced in both tools.

---

## Adding a Missing Filter

Filters live in `src/plugins/filter`, one module per category:

```rust
// 1. Implement the filter in the matching module, e.g. src/plugins/filter/strings.rs
fn my_filter(value: Value, arg: Option<String>) -> Result<String, Error> {
    // Implementation; return Err for input the filter cannot handle.
}

// 2. Register it in that module's register_filters()
env.add_filter("my_filter", my_filter);
```

Add unit tests next to the implementation and an engine-level case in
`tests/jinja2_filter_parity_tests.rs`, which renders through the production
`TemplateEngine`. Update this document in the same change.

Engine-local overrides in `src/template.rs` are reserved for names where
Ansible and Jinja2 disagree; adding one there shadows MiniJinja's built-in for
every playbook, so prefer a plugin module.
