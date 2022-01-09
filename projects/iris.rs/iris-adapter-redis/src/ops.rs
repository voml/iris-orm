//! Private Redis command helpers (KEY/SET/GET/DEL/TTL only -- no SCAN).

use redis::Commands;
use redis::Connection;

use crate::mapping::{KeyEncoding, KeyspaceMapping};
use crate::{Error, Result};

pub(crate) fn get_primary(
    conn: &mut Connection,
    map: &KeyspaceMapping,
    primary_key: &str,
) -> Result<Option<String>> {
    let key = map.redis_key(primary_key);
    let value: Option<String> = conn.get(&key).map_err(Error::Redis)?;
    if let Some(ref raw) = value {
        validate_encoding(map.encoding, raw)?;
    }
    Ok(value)
}

pub(crate) fn put_primary(
    conn: &mut Connection,
    map: &KeyspaceMapping,
    primary_key: &str,
    value: &str,
) -> Result<()> {
    validate_encoding(map.encoding, value)?;
    let key = map.redis_key(primary_key);
    match map.ttl_secs {
        Some(ttl) => {
            let _: () = conn.set_ex(&key, value, ttl).map_err(Error::Redis)?;
        }
        None => {
            let _: () = conn.set(&key, value).map_err(Error::Redis)?;
        }
    }
    Ok(())
}

pub(crate) fn put_primary_nx(
    conn: &mut Connection,
    map: &KeyspaceMapping,
    primary_key: &str,
    value: &str,
) -> Result<bool> {
    validate_encoding(map.encoding, value)?;
    let key = map.redis_key(primary_key);
    // SET NX [EX ttl]
    let mut cmd = redis::cmd("SET");
    cmd.arg(&key).arg(value).arg("NX");
    if let Some(ttl) = map.ttl_secs {
        cmd.arg("EX").arg(ttl);
    }
    let reply: Option<String> = cmd.query(conn).map_err(Error::Redis)?;
    Ok(reply.as_deref() == Some("OK"))
}

pub(crate) fn delete_primary(
    conn: &mut Connection,
    map: &KeyspaceMapping,
    primary_key: &str,
) -> Result<bool> {
    let key = map.redis_key(primary_key);
    let n: i64 = conn.del(&key).map_err(Error::Redis)?;
    Ok(n > 0)
}

pub(crate) fn ttl_primary(
    conn: &mut Connection,
    map: &KeyspaceMapping,
    primary_key: &str,
) -> Result<i64> {
    let key = map.redis_key(primary_key);
    let ttl: i64 = conn.ttl(&key).map_err(Error::Redis)?;
    Ok(ttl)
}

fn validate_encoding(encoding: KeyEncoding, value: &str) -> Result<()> {
    match encoding {
        KeyEncoding::Utf8String => Ok(()),
        KeyEncoding::JsonDocument => {
            serde_json::from_str::<serde_json::Value>(value).map_err(|e| {
                Error::Policy(format!("JsonDocument encoding requires valid JSON: {e}"))
            })?;
            Ok(())
        }
    }
}
