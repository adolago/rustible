//! Collection manipulation filters for Jinja2 templates.
//!
//! This module provides filters for working with lists, dictionaries, and
//! performing set operations, compatible with Ansible's Jinja2 collection filters.
//!
//! Filters that Jinja2 itself defines (`batch`, `slice`, `groupby`, `zip`,
//! `selectattr`, `rejectattr`) are left to MiniJinja's built-ins so their
//! semantics — every registered test, dotted attribute paths — stay
//! Jinja2-compatible.
//!
//! # Available Filters
//!
//! - `combine`: Merge dictionaries together
//! - `union`: Combine lists, removing duplicates
//! - `difference`: Elements in first list but not in second
//! - `intersect`: Elements common to both lists
//! - `symmetric_difference`: Elements in either list but not both
//! - `unique`: Remove duplicates from a list
//! - `flatten`: Flatten nested lists
//! - `zip_longest`: Like zip, but uses fillvalue for shorter lists
//! - `dict2items`: Convert dictionary to list of key-value pairs
//! - `items2dict`: Convert list of key-value pairs to dictionary
//! - `subelements`: Create combinations of items with subelements
//! - `map_attribute`: Extract attribute from list of objects
//! - `permutations` / `combinations`: Ordered and unordered selections
//! - `extract`: Look a key up in a container (pairs with `map`)
//! - `rekey_on_member`: Turn a list of dicts into a dict keyed by a member
//! - `random` / `shuffle`: Random selection and ordering, optionally seeded
//!
//! # Examples
//!
//! ```jinja2
//! {{ dict1 | combine(dict2) }}
//! {{ list1 | union(list2) }}
//! {{ list1 | difference(list2) }}
//! {{ data | dict2items }}
//! ```

use minijinja::value::{Kwargs, ValueKind};
use minijinja::{Environment, Value};
use std::collections::{BTreeMap, HashSet};

trait ValueSeqExt {
    fn as_seq(&self) -> Option<Vec<Value>>;
}

impl ValueSeqExt for Value {
    fn as_seq(&self) -> Option<Vec<Value>> {
        match self.kind() {
            ValueKind::Seq | ValueKind::Iterable => self.try_iter().ok().map(|iter| iter.collect()),
            _ => None,
        }
    }
}

/// Register all collection filters with the given environment.
pub fn register_filters(env: &mut Environment<'static>) {
    env.add_filter("combine", combine);
    env.add_filter("union", union);
    env.add_filter("difference", difference);
    env.add_filter("intersect", intersect);
    env.add_filter("symmetric_difference", symmetric_difference);
    env.add_filter("unique", unique);
    env.add_filter("flatten", flatten);
    env.add_filter("zip_longest", zip_longest);
    env.add_filter("dict2items", dict2items);
    env.add_filter("items2dict", items2dict);
    env.add_filter("subelements", subelements);
    env.add_filter("map_attribute", map_attribute);
    env.add_filter("product", product);
    env.add_filter("permutations", permutations);
    env.add_filter("combinations", combinations);
    env.add_filter("extract", extract);
    env.add_filter("rekey_on_member", rekey_on_member);
    env.add_filter("random", random_filter);
    env.add_filter("shuffle", shuffle);
}

/// Merge dictionaries together.
///
/// # Arguments
///
/// * `base` - The base dictionary
/// * `other` - The dictionary to merge into base
/// * `recursive` - Optional: merge nested dicts recursively (default: false)
/// * `list_merge` - Optional: how to merge lists ("replace", "keep", "append", "prepend", "append_rp", "prepend_rp")
///
/// # Returns
///
/// A new dictionary with values from `other` overlaid on `base`.
///
/// # Ansible Compatibility
///
/// Compatible with Ansible's `combine` filter.
fn combine(
    base: Value,
    other: Value,
    recursive: Option<bool>,
    list_merge: Option<String>,
) -> Value {
    let recursive = recursive.unwrap_or(false);
    let list_merge = list_merge.unwrap_or_else(|| "replace".to_string());

    if let (Some(base_obj), Some(other_obj)) = (base.as_object(), other.as_object()) {
        let mut result = BTreeMap::new();

        // First, add all keys from base
        if let Some(iter) = base_obj.try_iter_pairs() {
            for (key, val) in iter {
                result.insert(key.to_string(), val);
            }
        }

        // Then, overlay/merge keys from other
        if let Some(iter) = other_obj.try_iter_pairs() {
            for (key, other_val) in iter {
                let key_str = key.to_string();
                let merged_val = if recursive {
                    if let Some(base_val) = result.get(&key_str).cloned() {
                        merge_values(base_val, other_val, &list_merge)
                    } else {
                        other_val
                    }
                } else {
                    other_val
                };
                result.insert(key_str, merged_val);
            }
        }

        Value::from_iter(result)
    } else {
        base
    }
}

fn merge_values(base: Value, other: Value, list_merge: &str) -> Value {
    match (base.as_object(), other.as_object()) {
        (Some(base_obj), Some(other_obj)) => {
            // Both are dicts, merge recursively
            let mut result = BTreeMap::new();
            if let Some(iter) = base_obj.try_iter_pairs() {
                for (key, val) in iter {
                    result.insert(key.to_string(), val);
                }
            }
            if let Some(iter) = other_obj.try_iter_pairs() {
                for (key, other_val) in iter {
                    let key_str = key.to_string();
                    let merged = if let Some(base_val) = result.get(&key_str).cloned() {
                        merge_values(base_val, other_val, list_merge)
                    } else {
                        other_val
                    };
                    result.insert(key_str, merged);
                }
            }
            Value::from_iter(result)
        }
        _ => {
            // Handle list merging
            if let (Some(base_seq), Some(other_seq)) = (base.as_seq(), other.as_seq()) {
                match list_merge {
                    "append" => {
                        let mut result = base_seq;
                        result.extend(other_seq);
                        Value::from(result)
                    }
                    "prepend" => {
                        let mut result = other_seq;
                        result.extend(base_seq);
                        Value::from(result)
                    }
                    "append_rp" => {
                        // Append with remove duplicates from base that exist in other
                        let other_set: HashSet<String> =
                            other_seq.iter().map(|v| v.to_string()).collect();
                        let mut result: Vec<Value> = base_seq
                            .into_iter()
                            .filter(|v| !other_set.contains(&v.to_string()))
                            .collect();
                        result.extend(other_seq);
                        Value::from(result)
                    }
                    "prepend_rp" => {
                        // Prepend with remove duplicates from base that exist in other
                        let other_set: HashSet<String> =
                            other_seq.iter().map(|v| v.to_string()).collect();
                        let mut result = other_seq;
                        result.extend(
                            base_seq
                                .into_iter()
                                .filter(|v| !other_set.contains(&v.to_string())),
                        );
                        Value::from(result)
                    }
                    "keep" => base,
                    _ => other, // "replace" or default
                }
            } else {
                other
            }
        }
    }
}

/// Combine lists, removing duplicates.
///
/// # Arguments
///
/// * `list1` - First list
/// * `list2` - Second list
///
/// # Returns
///
/// A new list with unique elements from both lists.
///
/// # Ansible Compatibility
///
/// Compatible with Ansible's `union` filter.
fn union(list1: Value, list2: Value) -> Vec<Value> {
    let mut seen = HashSet::new();
    let mut result = Vec::new();

    if let Some(seq1) = list1.as_seq() {
        for item in seq1.iter() {
            let key = item.to_string();
            if seen.insert(key) {
                result.push(item.clone());
            }
        }
    }

    if let Some(seq2) = list2.as_seq() {
        for item in seq2.iter() {
            let key = item.to_string();
            if seen.insert(key) {
                result.push(item.clone());
            }
        }
    }

    result
}

/// Get elements in first list but not in second.
///
/// # Arguments
///
/// * `list1` - First list
/// * `list2` - Second list
///
/// # Returns
///
/// Elements that are in `list1` but not in `list2`.
///
/// # Ansible Compatibility
///
/// Compatible with Ansible's `difference` filter.
fn difference(list1: Value, list2: Value) -> Vec<Value> {
    let set2: HashSet<String> = list2
        .as_seq()
        .map(|s| s.iter().map(|v| v.to_string()).collect())
        .unwrap_or_default();

    list1
        .as_seq()
        .map(|s| {
            s.iter()
                .filter(|v| !set2.contains(&v.to_string()))
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}

/// Get elements common to both lists.
///
/// # Arguments
///
/// * `list1` - First list
/// * `list2` - Second list
///
/// # Returns
///
/// Elements that exist in both lists.
///
/// # Ansible Compatibility
///
/// Compatible with Ansible's `intersect` filter.
fn intersect(list1: Value, list2: Value) -> Vec<Value> {
    let set2: HashSet<String> = list2
        .as_seq()
        .map(|s| s.iter().map(|v| v.to_string()).collect())
        .unwrap_or_default();

    list1
        .as_seq()
        .map(|s| {
            s.iter()
                .filter(|v| set2.contains(&v.to_string()))
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}

/// Get elements in either list but not in both.
///
/// # Arguments
///
/// * `list1` - First list
/// * `list2` - Second list
///
/// # Returns
///
/// Elements unique to each list (XOR).
///
/// # Ansible Compatibility
///
/// Compatible with Ansible's `symmetric_difference` filter.
fn symmetric_difference(list1: Value, list2: Value) -> Vec<Value> {
    let set1: HashSet<String> = list1
        .as_seq()
        .map(|s| s.iter().map(|v| v.to_string()).collect())
        .unwrap_or_default();

    let set2: HashSet<String> = list2
        .as_seq()
        .map(|s| s.iter().map(|v| v.to_string()).collect())
        .unwrap_or_default();

    let mut result = Vec::new();

    if let Some(seq1) = list1.as_seq() {
        for item in seq1.iter() {
            if !set2.contains(&item.to_string()) {
                result.push(item.clone());
            }
        }
    }

    if let Some(seq2) = list2.as_seq() {
        for item in seq2.iter() {
            if !set1.contains(&item.to_string()) {
                result.push(item.clone());
            }
        }
    }

    result
}

/// Remove duplicates from a list.
///
/// # Arguments
///
/// * `list` - The list to deduplicate
/// * `case_sensitive` - Optional: case-sensitive comparison (default: true)
///
/// # Returns
///
/// A list with duplicate values removed, preserving order.
///
/// # Ansible Compatibility
///
/// Compatible with Ansible's `unique` filter.
/// Follow a dotted attribute path, as Jinja2's `attribute=` arguments do.
fn get_path(value: &Value, path: &str) -> Value {
    let mut current = value.clone();
    for segment in path.split('.') {
        if current.is_undefined() {
            return current;
        }
        current = match segment.parse::<usize>() {
            Ok(index) => current.get_item_by_index(index).unwrap_or(Value::UNDEFINED),
            Err(_) => current
                .get_item(&Value::from(segment))
                .unwrap_or(Value::UNDEFINED),
        };
    }
    current
}

fn unique(
    list: Value,
    case_sensitive: Option<bool>,
    kwargs: Kwargs,
) -> Result<Vec<Value>, minijinja::Error> {
    let attribute: Option<String> = kwargs.get("attribute")?;
    let case_sensitive = match kwargs.get::<Option<bool>>("case_sensitive")? {
        Some(from_kwargs) => from_kwargs,
        None => case_sensitive.unwrap_or(true),
    };
    kwargs.assert_all_used()?;

    let mut seen = HashSet::new();
    let mut result = Vec::new();

    if let Some(seq) = list.as_seq() {
        for item in seq.iter() {
            // With an attribute, uniqueness is decided by that member only.
            let compared = match &attribute {
                Some(attribute) => get_path(item, attribute),
                None => item.clone(),
            };
            let key = if case_sensitive {
                compared.to_string()
            } else {
                compared.to_string().to_lowercase()
            };
            if seen.insert(key) {
                result.push(item.clone());
            }
        }
    }

    Ok(result)
}

/// Flatten nested lists.
///
/// # Arguments
///
/// * `list` - The nested list to flatten
/// * `levels` - Optional: number of levels to flatten (default: all)
///
/// # Returns
///
/// A flattened list.
///
/// # Ansible Compatibility
///
/// Compatible with Ansible's `flatten` filter.
fn flatten(
    list: Value,
    levels: Option<i64>,
    kwargs: Kwargs,
) -> Result<Vec<Value>, minijinja::Error> {
    fn flatten_recursive(
        value: &Value,
        depth: i64,
        max_depth: Option<i64>,
        result: &mut Vec<Value>,
    ) {
        if let Some(max) = max_depth {
            if depth > max {
                result.push(value.clone());
                return;
            }
        }

        if let Some(seq) = value.as_seq() {
            for item in seq.iter() {
                flatten_recursive(item, depth + 1, max_depth, result);
            }
        } else {
            result.push(value.clone());
        }
    }

    // Ansible's flatten drops nulls unless asked to keep them.
    let skip_nulls: bool = kwargs.get::<Option<bool>>("skip_nulls")?.unwrap_or(true);
    kwargs.assert_all_used()?;

    let mut result = Vec::new();
    flatten_recursive(&list, 0, levels, &mut result);
    if skip_nulls {
        result.retain(|item| !item.is_none() && !item.is_undefined());
    }
    Ok(result)
}

/// Combine lists element-wise, filling shorter lists.
///
/// # Arguments
///
/// * `list1` - First list
/// * `list2` - Second list
/// * `fillvalue` - Optional: value to use for missing elements (default: none)
///
/// # Returns
///
/// A list of pairs, continuing to the longer list.
fn zip_longest(list1: Value, list2: Value, fillvalue: Option<Value>) -> Vec<Value> {
    let fillvalue = fillvalue.unwrap_or(Value::from(()));

    let seq1 = list1.as_seq().unwrap_or_default();
    let seq2 = list2.as_seq().unwrap_or_default();

    let max_len = seq1.len().max(seq2.len());
    let mut result = Vec::new();

    for i in 0..max_len {
        let a = seq1.get(i).cloned().unwrap_or_else(|| fillvalue.clone());
        let b = seq2.get(i).cloned().unwrap_or_else(|| fillvalue.clone());
        result.push(Value::from(vec![a, b]));
    }

    result
}

/// Convert dictionary to list of key-value pairs.
///
/// # Arguments
///
/// * `dict` - The dictionary to convert
/// * `key_name` - Optional: name for the key field (default: "key")
/// * `value_name` - Optional: name for the value field (default: "value")
///
/// # Returns
///
/// A list of objects with key and value fields.
///
/// # Ansible Compatibility
///
/// Compatible with Ansible's `dict2items` filter.
fn dict2items(dict: Value, key_name: Option<String>, value_name: Option<String>) -> Vec<Value> {
    let key_name = key_name.unwrap_or_else(|| "key".to_string());
    let value_name = value_name.unwrap_or_else(|| "value".to_string());

    if let Some(obj) = dict.as_object() {
        if let Some(iter) = obj.try_iter_pairs() {
            return iter
                .map(|(k, v)| Value::from_iter([(key_name.clone(), k), (value_name.clone(), v)]))
                .collect();
        }
    }
    Vec::new()
}

/// Convert list of key-value pairs to dictionary.
///
/// # Arguments
///
/// * `list` - The list of key-value objects
/// * `key_name` - Optional: name of the key field (default: "key")
/// * `value_name` - Optional: name of the value field (default: "value")
///
/// # Returns
///
/// A dictionary constructed from the key-value pairs.
///
/// # Ansible Compatibility
///
/// Compatible with Ansible's `items2dict` filter.
fn items2dict(list: Value, key_name: Option<String>, value_name: Option<String>) -> Value {
    let key_name = key_name.unwrap_or_else(|| "key".to_string());
    let value_name = value_name.unwrap_or_else(|| "value".to_string());

    if let Some(seq) = list.as_seq() {
        let mut result = BTreeMap::new();
        for item in seq.iter() {
            if let Some(obj) = item.as_object() {
                if let (Some(k), Some(v)) = (
                    obj.get_value(&Value::from(key_name.clone())),
                    obj.get_value(&Value::from(value_name.clone())),
                ) {
                    result.insert(k.to_string(), v);
                }
            }
        }
        Value::from_iter(result)
    } else {
        Value::from(BTreeMap::<String, Value>::new())
    }
}

/// Create combinations of items with subelements.
///
/// # Arguments
///
/// * `list` - List of objects
/// * `key` - Key of the subelement list
///
/// # Returns
///
/// A list of [item, subelement] pairs.
///
/// # Ansible Compatibility
///
/// Compatible with Ansible's `subelements` filter.
fn subelements(list: Value, key: String) -> Vec<Value> {
    let mut result = Vec::new();

    if let Some(seq) = list.as_seq() {
        for item in seq.iter() {
            if let Some(obj) = item.as_object() {
                if let Some(sub) = obj.get_value(&Value::from(key.clone())) {
                    if let Some(sub_seq) = sub.as_seq() {
                        for sub_item in sub_seq.iter() {
                            result.push(Value::from(vec![item.clone(), sub_item.clone()]));
                        }
                    }
                }
            }
        }
    }

    result
}

/// Extract attribute from list of objects.
///
/// # Arguments
///
/// * `list` - List of objects
/// * `attr` - Attribute to extract
/// * `default` - Optional default value if attribute is missing
///
/// # Returns
///
/// A list of attribute values.
fn map_attribute(list: Value, attr: String, default: Option<Value>) -> Vec<Value> {
    let default = default.unwrap_or(Value::UNDEFINED);

    if let Some(seq) = list.as_seq() {
        seq.iter()
            .map(|item| {
                if let Some(obj) = item.as_object() {
                    obj.get_value(&Value::from(attr.clone()))
                        .unwrap_or_else(|| default.clone())
                } else {
                    default.clone()
                }
            })
            .collect()
    } else {
        Vec::new()
    }
}

/// Compute Cartesian product of lists.
///
/// # Arguments
///
/// * `list1` - First list
/// * `list2` - Second list
///
/// # Returns
///
/// All combinations of elements from both lists.
fn product(list1: Value, list2: Value) -> Vec<Value> {
    let seq1 = list1.as_seq().unwrap_or_default();
    let seq2 = list2.as_seq().unwrap_or_default();

    let mut result = Vec::new();
    for a in &seq1 {
        for b in &seq2 {
            result.push(Value::from(vec![a.clone(), b.clone()]));
        }
    }
    result
}

/// Ordered selections of length `count` from a list.
///
/// # Ansible Compatibility
///
/// Matches `ansible.builtin.permutations`. With no length, permutations of the
/// full list are returned.
fn permutations(list: Value, count: Option<usize>) -> Vec<Value> {
    let Some(items) = list.as_seq() else {
        return Vec::new();
    };
    let count = count.unwrap_or(items.len());
    if count > items.len() {
        return Vec::new();
    }

    let mut result = Vec::new();
    let mut used = vec![false; items.len()];
    let mut current = Vec::with_capacity(count);
    permute(&items, count, &mut used, &mut current, &mut result);
    result
}

fn permute(
    items: &[Value],
    count: usize,
    used: &mut Vec<bool>,
    current: &mut Vec<Value>,
    result: &mut Vec<Value>,
) {
    if current.len() == count {
        result.push(Value::from(current.clone()));
        return;
    }
    for index in 0..items.len() {
        if used[index] {
            continue;
        }
        used[index] = true;
        current.push(items[index].clone());
        permute(items, count, used, current, result);
        current.pop();
        used[index] = false;
    }
}

/// Unordered selections of length `count` from a list.
///
/// # Ansible Compatibility
///
/// Matches `ansible.builtin.combinations`.
fn combinations(list: Value, count: usize) -> Vec<Value> {
    let Some(items) = list.as_seq() else {
        return Vec::new();
    };
    if count > items.len() {
        return Vec::new();
    }

    let mut result = Vec::new();
    let mut current = Vec::with_capacity(count);
    combine_indices(&items, count, 0, &mut current, &mut result);
    result
}

fn combine_indices(
    items: &[Value],
    count: usize,
    start: usize,
    current: &mut Vec<Value>,
    result: &mut Vec<Value>,
) {
    if current.len() == count {
        result.push(Value::from(current.clone()));
        return;
    }
    for index in start..items.len() {
        current.push(items[index].clone());
        combine_indices(items, count, index + 1, current, result);
        current.pop();
    }
}

/// Look up a key in a container, following further keys into nested values.
///
/// # Ansible Compatibility
///
/// Matches `ansible.builtin.extract`, whose usual form is
/// `{{ indexes | map('extract', container) | list }}`.
fn extract(key: Value, container: Value, morekeys: Option<Value>) -> Value {
    let mut current = lookup(&container, &key);
    let Some(morekeys) = morekeys else {
        return current;
    };

    let keys = match morekeys.as_seq() {
        Some(keys) => keys,
        None => vec![morekeys],
    };
    for key in keys {
        if current.is_undefined() {
            return current;
        }
        current = lookup(&current, &key);
    }
    current
}

fn lookup(container: &Value, key: &Value) -> Value {
    if let Some(index) = key.as_i64() {
        if matches!(container.kind(), ValueKind::Seq | ValueKind::Iterable) {
            return container
                .get_item_by_index(index.max(0) as usize)
                .unwrap_or(Value::UNDEFINED);
        }
    }
    container.get_item(key).unwrap_or(Value::UNDEFINED)
}

/// Turn a list of dicts into a dict keyed by one of their members.
///
/// # Arguments
///
/// * `key` - Member whose value becomes the dictionary key
/// * `duplicates` - `error` (default) or `overwrite`
///
/// # Ansible Compatibility
///
/// Matches `ansible.builtin.rekey_on_member`, including its duplicate-key
/// error.
fn rekey_on_member(
    list: Value,
    key: String,
    duplicates: Option<String>,
) -> Result<Value, minijinja::Error> {
    let duplicates = duplicates.unwrap_or_else(|| "error".to_string());
    if duplicates != "error" && duplicates != "overwrite" {
        return Err(minijinja::Error::new(
            minijinja::ErrorKind::InvalidOperation,
            format!(
                "rekey_on_member: duplicates must be 'error' or 'overwrite', got '{}'",
                duplicates
            ),
        ));
    }

    // Accept either a list of dicts or a dict of dicts, as Ansible does.
    let entries: Vec<Value> = match list.kind() {
        ValueKind::Map => list
            .try_iter()
            .map(|keys| {
                keys.filter_map(|k| list.get_item(&k).ok())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default(),
        _ => list.as_seq().unwrap_or_default(),
    };

    let mut result: BTreeMap<String, Value> = BTreeMap::new();
    for entry in entries {
        if entry.kind() != ValueKind::Map {
            return Err(minijinja::Error::new(
                minijinja::ErrorKind::InvalidOperation,
                "rekey_on_member: every element must be a dictionary",
            ));
        }
        let Ok(member) = entry.get_item(&Value::from(key.clone())) else {
            return Err(minijinja::Error::new(
                minijinja::ErrorKind::InvalidOperation,
                format!("rekey_on_member: element is missing key '{}'", key),
            ));
        };
        if member.is_undefined() {
            return Err(minijinja::Error::new(
                minijinja::ErrorKind::InvalidOperation,
                format!("rekey_on_member: element is missing key '{}'", key),
            ));
        }
        let member = member.to_string();
        if result.contains_key(&member) && duplicates == "error" {
            return Err(minijinja::Error::new(
                minijinja::ErrorKind::InvalidOperation,
                format!("rekey_on_member: duplicate key '{}'", member),
            ));
        }
        result.insert(member, entry);
    }

    Ok(Value::from_iter(result))
}

/// Pick a random element of a list, or a random number below a bound.
///
/// # Arguments
///
/// * `seed` - Makes the choice deterministic for a given seed
/// * `start` - Lower bound when the input is a number (default 0)
/// * `step` - Step between candidate numbers (default 1)
///
/// # Ansible Compatibility
///
/// Matches Ansible's `random` filter in shape and in being deterministic for a
/// given seed. The sequence itself differs from Python's `random`, so a seeded
/// run picks a stable value but not the same value Ansible would pick.
fn random_filter(value: Value, kwargs: Kwargs) -> Result<Value, minijinja::Error> {
    let seed: Option<Value> = kwargs.get("seed")?;
    let start: Option<i64> = kwargs.get("start")?;
    let step: Option<i64> = kwargs.get("step")?;
    kwargs.assert_all_used()?;

    let mut rng = SeededRng::new(seed.as_ref());

    if let Some(items) = value.as_seq() {
        if items.is_empty() {
            return Ok(Value::UNDEFINED);
        }
        let index = rng.next_below(items.len() as u64) as usize;
        return Ok(items[index].clone());
    }

    let end = value.as_i64().ok_or_else(|| {
        minijinja::Error::new(
            minijinja::ErrorKind::InvalidOperation,
            "random: input must be a list or a number",
        )
    })?;
    let start = start.unwrap_or(0);
    let step = step.unwrap_or(1).max(1);
    if end <= start {
        return Err(minijinja::Error::new(
            minijinja::ErrorKind::InvalidOperation,
            format!("random: start {} is not below end {}", start, end),
        ));
    }

    let candidates = ((end - start) as u64).div_ceil(step as u64);
    let offset = rng.next_below(candidates) as i64;
    Ok(Value::from(start + offset * step))
}

/// Shuffle a list.
///
/// # Ansible Compatibility
///
/// Matches Ansible's `shuffle` filter, with the same seeding caveat as
/// `random`: seeded runs are stable but not identical to Python's ordering.
fn shuffle(value: Value, kwargs: Kwargs) -> Result<Value, minijinja::Error> {
    let seed: Option<Value> = kwargs.get("seed")?;
    kwargs.assert_all_used()?;

    let Some(mut items) = value.as_seq() else {
        // Ansible returns non-sequences unchanged.
        return Ok(value);
    };

    let mut rng = SeededRng::new(seed.as_ref());
    for index in (1..items.len()).rev() {
        let swap = rng.next_below(index as u64 + 1) as usize;
        items.swap(index, swap);
    }
    Ok(Value::from(items))
}

/// SplitMix64, used so seeded `random`/`shuffle` results are reproducible
/// across runs and platforms.
struct SeededRng {
    state: u64,
}

impl SeededRng {
    fn new(seed: Option<&Value>) -> Self {
        let state = match seed {
            Some(seed) => {
                use std::hash::{Hash, Hasher};
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                seed.to_string().hash(&mut hasher);
                hasher.finish()
            }
            None => {
                use rand::Rng;
                rand::rng().random()
            }
        };
        Self { state }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn next_below(&mut self, bound: u64) -> u64 {
        if bound == 0 {
            return 0;
        }
        self.next_u64() % bound
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
    fn test_permutations() {
        assert_eq!(render("{{ [1, 2, 3] | permutations(2) | length }}"), "6");
        assert_eq!(
            render("{{ [1, 2, 3] | permutations(2) | first | join('') }}"),
            "12"
        );
        assert_eq!(render("{{ [1, 2] | permutations | length }}"), "2");
        assert_eq!(render("{{ [1, 2] | permutations(3) | length }}"), "0");
    }

    #[test]
    fn test_combinations() {
        assert_eq!(render("{{ [1, 2, 3] | combinations(2) | length }}"), "3");
        assert_eq!(
            render("{{ [1, 2, 3] | combinations(2) | last | join('') }}"),
            "23"
        );
    }

    #[test]
    fn test_extract() {
        assert_eq!(
            render("{{ [0, 2] | map('extract', ['a', 'b', 'c']) | join(',') }}"),
            "a,c"
        );
        assert_eq!(
            render("{{ 'x' | extract({'x': {'y': 'found'}}, 'y') }}"),
            "found"
        );
    }

    #[test]
    fn test_rekey_on_member() {
        let people = "[{'name': 'alice', 'age': 30}, {'name': 'bob', 'age': 40}]";
        assert_eq!(
            render(&format!(
                "{{{{ {} | rekey_on_member('name') | length }}}}",
                people
            )),
            "2"
        );
        assert_eq!(
            render(&format!(
                "{{{{ ({} | rekey_on_member('name')).alice.age }}}}",
                people
            )),
            "30"
        );
    }

    #[test]
    fn test_rekey_on_member_reports_duplicates() {
        let people = "[{'name': 'alice'}, {'name': 'alice'}]";
        assert!(
            render_err(&format!("{{{{ {} | rekey_on_member('name') }}}}", people))
                .contains("duplicate key")
        );
        assert_eq!(
            render(&format!(
                "{{{{ {} | rekey_on_member('name', 'overwrite') | length }}}}",
                people
            )),
            "1"
        );
    }

    #[test]
    fn test_rekey_on_member_requires_the_key() {
        assert!(render_err("{{ [{'other': 1}] | rekey_on_member('name') }}")
            .contains("missing key 'name'"));
    }

    #[test]
    fn test_random_is_stable_for_a_seed() {
        let first = render("{{ [1, 2, 3, 4, 5] | random(seed='host1') }}");
        let again = render("{{ [1, 2, 3, 4, 5] | random(seed='host1') }}");
        assert_eq!(first, again);
        assert!(["1", "2", "3", "4", "5"].contains(&first.as_str()));
    }

    #[test]
    fn test_random_number_bounds() {
        for _ in 0..25 {
            let value: i64 = render("{{ 10 | random }}").parse().unwrap();
            assert!((0..10).contains(&value), "out of range: {}", value);
        }
        let stepped: i64 = render("{{ 30 | random(start=10, step=10) }}")
            .parse()
            .unwrap();
        assert!([10, 20].contains(&stepped), "unexpected value: {}", stepped);
    }

    #[test]
    fn test_shuffle_keeps_every_element() {
        let shuffled = render("{{ [1, 2, 3, 4, 5] | shuffle(seed='host1') | sort | join(',') }}");
        assert_eq!(shuffled, "1,2,3,4,5");
        let first = render("{{ [1, 2, 3, 4, 5] | shuffle(seed='host1') | join(',') }}");
        let again = render("{{ [1, 2, 3, 4, 5] | shuffle(seed='host1') | join(',') }}");
        assert_eq!(first, again);
    }

    #[test]
    fn test_combine_basic() {
        let base = Value::from_iter([
            ("a".to_string(), Value::from(1)),
            ("b".to_string(), Value::from(2)),
        ]);
        let other = Value::from_iter([
            ("b".to_string(), Value::from(3)),
            ("c".to_string(), Value::from(4)),
        ]);

        let result = combine(base, other, None, None);
        assert!(!result.is_undefined());
    }

    #[test]
    fn test_union() {
        let list1 = Value::from(vec![Value::from(1), Value::from(2), Value::from(3)]);
        let list2 = Value::from(vec![Value::from(2), Value::from(3), Value::from(4)]);

        let result = union(list1, list2);
        assert_eq!(result.len(), 4);
    }

    #[test]
    fn test_difference() {
        let list1 = Value::from(vec![Value::from(1), Value::from(2), Value::from(3)]);
        let list2 = Value::from(vec![Value::from(2), Value::from(3), Value::from(4)]);

        let result = difference(list1, list2);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].to_string(), "1");
    }

    #[test]
    fn test_intersect() {
        let list1 = Value::from(vec![Value::from(1), Value::from(2), Value::from(3)]);
        let list2 = Value::from(vec![Value::from(2), Value::from(3), Value::from(4)]);

        let result = intersect(list1, list2);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_symmetric_difference() {
        let list1 = Value::from(vec![Value::from(1), Value::from(2), Value::from(3)]);
        let list2 = Value::from(vec![Value::from(2), Value::from(3), Value::from(4)]);

        let result = symmetric_difference(list1, list2);
        assert_eq!(result.len(), 2); // 1 and 4
    }

    #[test]
    fn test_unique() {
        assert_eq!(
            render("{{ [1, 2, 2, 3, 1] | unique | join(',') }}"),
            "1,2,3"
        );
        assert_eq!(render("{{ ['CA', 'ca'] | unique | join(',') }}"), "CA,ca");
        assert_eq!(
            render("{{ ['CA', 'ca'] | unique(case_sensitive=false) | join(',') }}"),
            "CA"
        );
        assert_eq!(
            render(
                "{{ [{'k': 1, 'v': 'a'}, {'k': 1, 'v': 'b'}] | unique(attribute='k') | length }}"
            ),
            "1"
        );
    }

    #[test]
    fn test_flatten() {
        assert_eq!(
            render("{{ [1, [2, 3], [4, [5]]] | flatten | join(',') }}"),
            "1,2,3,4,5"
        );
        assert_eq!(
            render("{{ [1, [2, [3]]] | flatten(1) | length }}"),
            "3",
            "one level of flattening leaves the inner list intact"
        );
        assert_eq!(
            render("{{ [1, none, [2, none]] | flatten | join(',') }}"),
            "1,2",
            "Ansible's flatten drops nulls by default"
        );
        assert_eq!(
            render("{{ [1, none] | flatten(skip_nulls=false) | length }}"),
            "2"
        );
    }

    #[test]
    fn test_dict2items() {
        let dict = Value::from_iter([
            ("a".to_string(), Value::from(1)),
            ("b".to_string(), Value::from(2)),
        ]);

        let result = dict2items(dict, None, None);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_items2dict() {
        let items = Value::from(vec![
            Value::from_iter([
                ("key".to_string(), Value::from("a")),
                ("value".to_string(), Value::from(1)),
            ]),
            Value::from_iter([
                ("key".to_string(), Value::from("b")),
                ("value".to_string(), Value::from(2)),
            ]),
        ]);

        let result = items2dict(items, None, None);
        assert!(!result.is_undefined());
    }

    #[test]
    fn test_product() {
        let list1 = Value::from(vec![Value::from("a"), Value::from("b")]);
        let list2 = Value::from(vec![Value::from(1), Value::from(2)]);

        let result = product(list1, list2);
        assert_eq!(result.len(), 4); // (a,1), (a,2), (b,1), (b,2)
    }
}
