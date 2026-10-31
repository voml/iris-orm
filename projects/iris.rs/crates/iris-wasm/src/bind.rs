//! Bind `$name` parameters into VOS source before parse/plan (Rust authority).
//!
//! Shared by N-API and WASM hosts. TypeScript must not interpolate values into VOS text.
//!
//! VOS has no literal for the `bytes` / `vector` / `file` payloads, so those
//! parameters cannot become source text. They stay as `$name` in the bound
//! source and are carried as typed [`Value`]s, which a backend that binds
//! typed values (YYDB) applies directly. Every other scalar is substituted as
//! the VOS literal of its kind.

use std::collections::BTreeMap;

use iris_types::Value;
use serde_json::Value as JsonValue;

/// Result of binding `$name` parameters into a VOS source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundParameters {
    /// Source text where every text-bindable scalar replaced its `$name`.
    ///
    /// Payload placeholders stay verbatim so a typed-binding backend resolves
    /// them from [`Self::value_parameters`] instead of from text.
    pub source: String,
    /// Payload parameters by name, decoded from the tagged JSON wire.
    pub value_parameters: BTreeMap<String, Value>,
}

impl BoundParameters {
    /// An unparameterised source: no substitution, no typed values.
    pub fn unbound(source: &str) -> Self {
        Self { source: source.to_owned(), value_parameters: BTreeMap::new() }
    }

    /// The source text, refusing to hand typed payload values to a text-only backend.
    pub fn text_only(&self) -> Result<&str, String> {
        match self.value_parameters.keys().next() {
            Some(name) => Err(format!(
                "parameter `${name}` carries a VOS `bytes`/`vector`/`file` payload, which needs a datasource that binds typed values (yydb)"
            )),
            None => Ok(&self.source),
        }
    }
}

/// Substitute `$ident` placeholders in `source` using a JSON object of parameters.
///
/// Text-only variant: a payload parameter has no VOS literal and is rejected.
pub fn bind_parameters(source: &str, parameters_json: &str) -> Result<String, String> {
    let bound = bind_parameters_typed(source, parameters_json)?;
    Ok(bound.text_only()?.to_owned())
}

/// Substitute scalar parameters and keep payload parameters as typed values.
pub fn bind_parameters_typed(source: &str, parameters_json: &str) -> Result<BoundParameters, String> {
    let params: JsonValue = serde_json::from_str(parameters_json).map_err(|err| format!("invalid parameters JSON: {err}"))?;
    let obj = params.as_object().ok_or_else(|| "parameters must be a JSON object".to_string())?;

    let mut out = String::with_capacity(source.len());
    let mut value_parameters: BTreeMap<String, Value> = BTreeMap::new();
    let bytes = source.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'$' {
            let start = i + 1;
            let mut end = start;
            while end < bytes.len() && is_ident_byte(bytes[end]) {
                end += 1;
            }
            if end > start {
                let name = std::str::from_utf8(&bytes[start..end]).map_err(|err| err.to_string())?;
                let value = obj.get(name).ok_or_else(|| format!("unbound VOS parameter `${name}`"))?;
                if let Some(kind) = iris_types::tagged_json_kind(value) {
                    let typed = match kind {
                        "bytes" => iris_types::bytes_from_tagged_json(value)
                            .map(Value::Bytes)
                            .ok_or_else(|| format!("parameter `${name}` is tagged `bytes` without canonical lowercase hex"))?,
                        "vector" => iris_types::vector_from_tagged_json(value)
                            .map(Value::Vector)
                            .ok_or_else(|| format!("parameter `${name}` is tagged `vector` with a malformed hex, length or dimension"))?,
                        "file" => iris_types::file_from_tagged_json(value)
                            .map(Value::File)
                            .ok_or_else(|| format!("parameter `${name}` is tagged `file` with a malformed descriptor"))?,
                        other => return Err(format!("parameter `${name}` claims unknown tagged carrier `{other}`")),
                    };
                    value_parameters.insert(name.to_owned(), typed);
                    out.push('$');
                    out.push_str(name);
                }
                else {
                    out.push_str(&encode_literal(value)?);
                }
                i = end;
                continue;
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    Ok(BoundParameters { source: out, value_parameters })
}

fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn encode_literal(value: &JsonValue) -> Result<String, String> {
    match value {
        JsonValue::Null => Ok("null".into()),
        JsonValue::Bool(b) => Ok(if *b { "true".into() } else { "false".into() }),
        JsonValue::Number(n) => {
            let text = n.to_string();
            let is_integer_token = !text.contains('.') && !text.contains('e') && !text.contains('E');
            if is_integer_token && n.as_i64().is_none() {
                return Err(format!("integer parameter `{text}` is outside the exact VOS `i64` range"));
            }
            Ok(text)
        }
        JsonValue::String(s) => Ok(format!("\"{}\"", escape_vos_string(s))),
        JsonValue::Array(_) | JsonValue::Object(_) => {
            Err("parameter values must be scalar (null|bool|number|string) or a tagged `bytes`/`vector`/`file` value".into())
        }
    }
}

fn escape_vos_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binds_scalars() {
        let source = "User.filter(x => x.active == $where_active).take($take).collect()";
        let bound = bind_parameters(source, r#"{"where_active":true,"take":20}"#).unwrap();
        assert_eq!(bound, "User.filter(x => x.active == true).take(20).collect()");
    }

    #[test]
    fn escapes_strings() {
        let bound = bind_parameters("User.filter(x => x.name == $n)", r#"{"n":"a\"b"}"#).unwrap();
        assert_eq!(bound, r#"User.filter(x => x.name == "a\"b")"#);
    }

    #[test]
    fn rejects_unbound() {
        let err = bind_parameters("User.filter(x => x.id == $missing)", "{}").unwrap_err();
        assert!(err.contains("unbound"));
    }

    #[test]
    fn keeps_tagged_bytes_as_a_typed_parameter() {
        let source = "Blob::insert({ blob_id: $blob_id, body: $body })";
        let bound = bind_parameters_typed(source, r#"{"blob_id":7,"body":{"$yydb":"bytes","hex":"00ff41"}}"#).unwrap();
        assert_eq!(bound.source, "Blob::insert({ blob_id: 7, body: $body })");
        assert_eq!(bound.value_parameters.get("body"), Some(&Value::Bytes(vec![0x00, 0xff, 0x41])));
    }

    #[test]
    fn text_only_binding_refuses_bytes() {
        let err = bind_parameters("Blob::insert({ body: $body })", r#"{"body":{"$yydb":"bytes","hex":""}}"#).unwrap_err();
        assert!(err.contains("`bytes`"), "{err}");
        assert!(err.contains("`$body`"), "{err}");
    }

    #[test]
    fn rejects_malformed_tagged_bytes() {
        let err = bind_parameters_typed("Blob::insert({ body: $body })", r#"{"body":{"$yydb":"bytes","hex":"0"}}"#).unwrap_err();
        assert!(err.contains("lowercase hex"), "{err}");
    }

    #[test]
    fn keeps_tagged_vector_as_a_typed_parameter() {
        let source = "Doc::insert({ id: $id, embedding: $embedding })";
        let bound =
            bind_parameters_typed(source, r#"{"id":3,"embedding":{"$yydb":"vector","dimension":2,"components":"0000803f00000040"}}"#).unwrap();
        assert_eq!(bound.source, "Doc::insert({ id: 3, embedding: $embedding })");
        assert_eq!(
            bound.value_parameters.get("embedding"),
            Some(&Value::Vector(iris_types::VectorValue { dimension: 2, components: vec![1.0, 2.0] }))
        );
    }

    #[test]
    fn keeps_tagged_file_as_a_typed_parameter() {
        let source = "Doc::insert({ attachment: $attachment })";
        let bound = bind_parameters_typed(
            source,
            r#"{"attachment":{"$yydb":"file","digest":"a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3","octets":10,"name":"note.txt","mediaType":"text/plain","manifest":{"hash":"b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4","size":128}}}"#,
        )
        .unwrap();
        assert_eq!(bound.source, "Doc::insert({ attachment: $attachment })");
        let expected = Value::File(iris_types::FileDescriptor {
            digest: "a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3".into(),
            octets: 10,
            name: Some("note.txt".into()),
            media_type: Some("text/plain".into()),
            manifest: iris_types::FileManifestRef {
                hash: "b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4".into(),
                size: 128,
            },
        });
        assert_eq!(bound.value_parameters.get("attachment"), Some(&expected));
    }

    #[test]
    fn rejects_malformed_tagged_vector_and_unknown_carrier() {
        let err =
            bind_parameters_typed("Doc::insert({ e: $e })", r#"{"e":{"$yydb":"vector","dimension":3,"components":"0000803f"}}"#).unwrap_err();
        assert!(err.contains("dimension"), "{err}");
        let err = bind_parameters_typed("Doc::insert({ e: $e })", r#"{"e":{"$yydb":"image","hex":""}}"#).unwrap_err();
        assert!(err.contains("unknown tagged carrier"), "{err}");
    }
}
