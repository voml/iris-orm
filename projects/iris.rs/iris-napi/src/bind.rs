//! Bind `$name` parameters into VOS source before parse/plan (Rust authority).
//!
//! TypeScript must not interpolate values into VOS text. Values arrive as JSON
//! and are encoded here as VOS literals.

use serde_json::Value as JsonValue;

/// Substitute `$ident` placeholders in `source` using a JSON object of parameters.
pub fn bind_parameters(source: &str, parameters_json: &str) -> Result<String, String> {
    let params: JsonValue = serde_json::from_str(parameters_json)
        .map_err(|err| format!("invalid parameters JSON: {err}"))?;
    let obj = params
        .as_object()
        .ok_or_else(|| "parameters must be a JSON object".to_string())?;

    let mut out = String::with_capacity(source.len());
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
                let name =
                    std::str::from_utf8(&bytes[start..end]).map_err(|err| err.to_string())?;
                let value = obj
                    .get(name)
                    .ok_or_else(|| format!("unbound VOS parameter `${name}`"))?;
                out.push_str(&encode_literal(value)?);
                i = end;
                continue;
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    Ok(out)
}

fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn encode_literal(value: &JsonValue) -> Result<String, String> {
    match value {
        JsonValue::Null => Ok("null".into()),
        JsonValue::Bool(b) => Ok(if *b { "true".into() } else { "false".into() }),
        JsonValue::Number(n) => Ok(n.to_string()),
        JsonValue::String(s) => Ok(format!("\"{}\"", escape_vos_string(s))),
        JsonValue::Array(_) | JsonValue::Object(_) => {
            Err("parameter values must be scalar (null|bool|number|string); nested objects are not VOS literals".into())
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
        assert_eq!(
            bound,
            "User.filter(x => x.active == true).take(20).collect()"
        );
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
}
