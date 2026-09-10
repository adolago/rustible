//! YAML loading helpers shared by every user-facing YAML entry point.
//!
//! Ansible resolves YAML merge keys (`<<: *anchor`) in playbooks, vars files,
//! inventories and role files alike. `serde_yaml` resolves anchors and aliases
//! on its own but leaves `<<` as an ordinary key, so every place that loads
//! user YAML goes through [`from_str`] or [`resolve_merge_keys`].

use serde::de::DeserializeOwned;

/// Parse YAML into `T`, resolving merge keys first.
pub fn from_str<T: DeserializeOwned>(yaml: &str) -> Result<T, serde_yaml::Error> {
    let value: serde_yaml::Value = serde_yaml::from_str(yaml)?;
    serde_yaml::from_value(resolve_merge_keys(value))
}

/// Parse a multi-document YAML stream into `T` values, resolving merge keys.
pub fn from_str_multi<T: DeserializeOwned>(yaml: &str) -> Result<Vec<T>, serde_yaml::Error> {
    serde_yaml::Deserializer::from_str(yaml)
        .map(|document| {
            let value = serde_yaml::Value::deserialize(document)?;
            serde_yaml::from_value(resolve_merge_keys(value))
        })
        .collect()
}

use serde::Deserialize;

/// Expand YAML merge keys (`<<`) into the surrounding mapping.
///
/// Explicit keys win over merged ones, and later sources in a merge sequence
/// win over earlier ones, matching the YAML 1.1 merge-key specification that
/// Ansible follows.
pub fn resolve_merge_keys(value: serde_yaml::Value) -> serde_yaml::Value {
    match value {
        serde_yaml::Value::Mapping(mapping) => {
            let mut merged = serde_yaml::Mapping::new();
            let mut explicit = Vec::new();

            for (key, value) in mapping {
                if matches!(&key, serde_yaml::Value::String(s) if s == "<<") {
                    match value {
                        serde_yaml::Value::Mapping(_) => {
                            if let serde_yaml::Value::Mapping(source) = resolve_merge_keys(value) {
                                for (k, v) in source {
                                    merged.insert(k, v);
                                }
                            }
                        }
                        serde_yaml::Value::Sequence(seq) => {
                            for item in seq {
                                if let serde_yaml::Value::Mapping(source) = resolve_merge_keys(item)
                                {
                                    for (k, v) in source {
                                        merged.insert(k, v);
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                } else {
                    explicit.push((key, value));
                }
            }

            for (key, value) in explicit {
                merged.insert(key, resolve_merge_keys(value));
            }

            serde_yaml::Value::Mapping(merged)
        }
        serde_yaml::Value::Sequence(seq) => {
            serde_yaml::Value::Sequence(seq.into_iter().map(resolve_merge_keys).collect())
        }
        serde_yaml::Value::Tagged(tagged) => {
            let mut tagged = *tagged;
            tagged.value = resolve_merge_keys(tagged.value);
            serde_yaml::Value::Tagged(Box::new(tagged))
        }
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn test_merge_key_from_anchor() {
        let yaml = r#"
defaults: &defaults
  port: 80
  user: www

site:
  <<: *defaults
  port: 8080
"#;
        let parsed: BTreeMap<String, BTreeMap<String, serde_yaml::Value>> = from_str(yaml).unwrap();
        let site = &parsed["site"];
        // The explicit key wins over the merged one.
        assert_eq!(site["port"], serde_yaml::Value::from(8080));
        assert_eq!(site["user"], serde_yaml::Value::from("www"));
    }

    #[test]
    fn test_merge_key_sequence_order() {
        let yaml = r#"
base: &base
  a: 1
  b: 1
override: &override
  b: 2

result:
  <<: [*base, *override]
"#;
        let parsed: BTreeMap<String, BTreeMap<String, serde_yaml::Value>> = from_str(yaml).unwrap();
        let result = &parsed["result"];
        assert_eq!(result["a"], serde_yaml::Value::from(1));
        // Later entries in the merge sequence win.
        assert_eq!(result["b"], serde_yaml::Value::from(2));
    }

    #[test]
    fn test_merge_keys_resolve_in_nested_sequences() {
        let yaml = r#"
common: &common
  become: true

tasks:
  - <<: *common
    name: first
"#;
        let parsed: BTreeMap<String, serde_yaml::Value> = from_str(yaml).unwrap();
        let tasks = parsed["tasks"].as_sequence().unwrap();
        let first = tasks[0].as_mapping().unwrap();
        assert_eq!(
            first.get(serde_yaml::Value::from("become")),
            Some(&serde_yaml::Value::from(true))
        );
        assert_eq!(
            first.get(serde_yaml::Value::from("name")),
            Some(&serde_yaml::Value::from("first"))
        );
    }

    #[test]
    fn test_multi_document_stream() {
        let yaml = r#"
base: &base
  a: 1
merged:
  <<: *base
---
other: 2
"#;
        let documents: Vec<serde_yaml::Value> = from_str_multi(yaml).unwrap();
        assert_eq!(documents.len(), 2);
        let merged = documents[0]
            .get("merged")
            .and_then(|value| value.get("a"))
            .unwrap();
        assert_eq!(merged, &serde_yaml::Value::from(1));
    }
}
