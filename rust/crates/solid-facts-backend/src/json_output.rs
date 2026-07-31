use serde::Serialize;

pub(crate) fn go_compatible<T: Serialize>(
    value: &T,
    pretty: bool,
) -> Result<Vec<u8>, serde_json::Error> {
    let encoded = if pretty {
        serde_json::to_string_pretty(value)?
    } else {
        serde_json::to_string(value)?
    };
    // Go's encoding/json escapes these code points by default. Keeping that
    // behavior makes Rust and Go snapshots/contracts byte-for-byte
    // interchangeable during the additive migration.
    Ok(encoded
        .replace('&', "\\u0026")
        .replace('<', "\\u003c")
        .replace('>', "\\u003e")
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029")
        .into_bytes())
}
