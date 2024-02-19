use serde_json::Value;

// Ref https://github.com/paritytech/polkadot-sdk/blob/2a32b5c9379e6daefd2061a8fb5b4d972d3460a1/substrate/client/chain-spec/src/json_patch.rs#L33
/// Recursively merges two JSON objects, `a` and `b`, into a single object.
///
/// If a key exists in both objects, the value from `b` will override the value from `a`.
/// If a key exists in `b` with a `null` value, it will be removed from `a`.
/// If a key exists only in `b` and not in `a`, it will be added to `a`.
///
/// # Arguments
///
/// * `a` - A mutable reference to the target JSON object to merge into.
/// * `b` - The JSON object to merge with `a`.
pub fn merge(a: &mut Value, b: Value) {
    match (a, b) {
        (Value::Object(a), Value::Object(b)) => {
            for (k, v) in b {
                if v.is_null() {
                    a.remove(&k);
                } else {
                    merge(a.entry(k).or_insert(Value::Null), v);
                }
            }
        }
        (a, b) => *a = b,
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test1_simple_merge() {
        let mut j1 = json!({ "a":123 });
        merge(&mut j1, json!({ "b":256 }));
        assert_eq!(j1, json!({ "a":123, "b":256 }));
    }

    #[test]
    fn test2_patch_simple_merge_nested() {
        let mut j1 = json!({
            "a": {
                "name": "xxx",
                "value": 123
            },
            "b": { "c" : { "inner_name": "yyy" } }
        });

        let j2 = json!({
            "a": {
                "keys": ["a", "b", "c" ]
            }
        });

        merge(&mut j1, j2);
        assert_eq!(
            j1,
            json!({"a":{"keys":["a","b","c"],"name":"xxx","value":123}, "b": { "c" : { "inner_name": "yyy" } }})
        );
    }

    #[test]
    fn test3_patch_overrides_existing_keys() {
        let mut j1 = json!({
            "a": {
                "name": "xxx",
                "value": 123,
                "keys": ["d"]

            }
        });

        let j2 = json!({
            "a": {
                "keys": ["a", "b", "c" ]
            }
        });

        merge(&mut j1, j2);
        assert_eq!(
            j1,
            json!({"a":{"keys":["a","b","c"],"name":"xxx","value":123}})
        );
    }

    #[test]
    fn test4_patch_overrides_existing_keys() {
        let mut j1 = json!({
            "a": {
                "name": "xxx",
                "value": 123,
                "b" : {
                    "inner_name": "yyy"
                }
            }
        });

        let j2 = json!({
            "a": {
                "name": "new_name",
                "b" : {
                    "inner_name": "inner_new_name"
                }
            }
        });

        merge(&mut j1, j2);
        assert_eq!(
            j1,
            json!({ "a": {"name":"new_name", "value":123, "b" : { "inner_name": "inner_new_name" }} })
        );
    }

    #[test]
    fn test5_patch_overrides_existing_nested_keys() {
        let mut j1 = json!({
            "a": {
                "name": "xxx",
                "value": 123,
                "b": {
                    "c": {
                        "d": {
                            "name": "yyy",
                            "value": 256
                        }
                    }
                }
            },
        });

        let j2 = json!({
            "a": {
                "value": 456,
                "b": {
                    "c": {
                        "d": {
                            "name": "new_name"
                        }
                    }
                }
            }
        });

        merge(&mut j1, j2);
        assert_eq!(
            j1,
            json!({ "a": {"name":"xxx", "value":456, "b": { "c": { "d": { "name": "new_name", "value": 256 }}}}})
        );
    }

    #[test]
    fn test6_patch_removes_keys_if_null() {
        let mut j1 = json!({
            "a": {
                "name": "xxx",
                "value": 123,
                "enum_variant_1": {
                    "name": "yyy",
                }
            },
        });

        let j2 = json!({
            "a": {
                "value": 456,
                "enum_variant_1": null,
                "enum_variant_2": 32,
            }
        });

        merge(&mut j1, j2);
        assert_eq!(
            j1,
            json!({ "a": {"name":"xxx", "value":456, "enum_variant_2": 32 }})
        );
    }
}
