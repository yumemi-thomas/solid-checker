//! Shared structural limits for every untrusted package-contract JSON family.
//!
//! Byte limits alone bound input storage but not the number of semantic nodes
//! created by tiny values, while serde's recursion limit does not constrain
//! wide arrays or oversized strings. This module makes those limits explicit
//! before a wire document is deserialized into its protocol-specific model.

use serde::de::DeserializeOwned;

#[derive(Clone, Copy)]
pub(crate) struct Limits {
    pub bytes: usize,
    pub depth: usize,
    pub nodes: usize,
    pub string_bytes: usize,
}

pub(crate) fn decode<T: DeserializeOwned>(bytes: &[u8], limits: Limits) -> Result<T, String> {
    let value = value(bytes, limits)?;
    serde_json::from_value(value).map_err(|error| error.to_string())
}

pub(crate) fn value(bytes: &[u8], limits: Limits) -> Result<serde_json::Value, String> {
    if bytes.is_empty() {
        return Err("JSON document is empty".into());
    }
    if bytes.len() > limits.bytes {
        return Err(format!(
            "JSON document exceeds the {} byte resource limit",
            limits.bytes
        ));
    }
    let value = serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
    let mut nodes = 0usize;
    validate(&value, 0, &mut nodes, limits)?;
    Ok(value)
}

fn validate(
    value: &serde_json::Value,
    depth: usize,
    nodes: &mut usize,
    limits: Limits,
) -> Result<(), String> {
    if depth > limits.depth {
        return Err(format!(
            "JSON container depth exceeds the {} level resource limit",
            limits.depth
        ));
    }
    *nodes = nodes
        .checked_add(1)
        .ok_or_else(|| "JSON node count overflowed".to_owned())?;
    if *nodes > limits.nodes {
        return Err(format!(
            "JSON node count exceeds the {} node resource limit",
            limits.nodes
        ));
    }
    match value {
        serde_json::Value::String(value) => validate_string(value, limits),
        serde_json::Value::Array(items) => {
            for item in items {
                validate(item, depth + 1, nodes, limits)?;
            }
            Ok(())
        }
        serde_json::Value::Object(fields) => {
            for (name, value) in fields {
                validate_string(name, limits)?;
                validate(value, depth + 1, nodes, limits)?;
            }
            Ok(())
        }
        serde_json::Value::Null | serde_json::Value::Bool(_) | serde_json::Value::Number(_) => {
            Ok(())
        }
    }
}

fn validate_string(value: &str, limits: Limits) -> Result<(), String> {
    if value.len() > limits.string_bytes {
        Err(format!(
            "JSON string exceeds the {} byte resource limit",
            limits.string_bytes
        ))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIMITS: Limits = Limits {
        bytes: 128,
        depth: 3,
        nodes: 6,
        string_bytes: 8,
    };

    #[test]
    fn rejects_each_independent_resource_dimension() {
        assert!(value(&[b' '; LIMITS.bytes + 1], LIMITS).is_err());
        assert!(value(br#"[[[[0]]]]"#, LIMITS).is_err());
        assert!(value(br#"[0,1,2,3,4,5]"#, LIMITS).is_err());
        assert!(value(br#""123456789""#, LIMITS).is_err());
        assert!(value(br#"{"123456789":0}"#, LIMITS).is_err());
    }

    #[test]
    fn decodes_only_after_the_tree_is_bounded() {
        let decoded: Vec<u8> = decode(br#"[1,2,3]"#, LIMITS).unwrap();
        assert_eq!(decoded, vec![1, 2, 3]);
    }
}
