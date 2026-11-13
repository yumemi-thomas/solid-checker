use std::{
    env, fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

/// ADR 0970: the owner emission takes a function a reference enters for a layer like any other, so an owner operation or a render effect in an exported component no
/// tag renders is an uncertifiable SC4001 row (`owner-callers-unenumerated`). A test whose subject is another rule leaves those rows out.
#[allow(dead_code)] // the process modules that include this one and keep those rows leave it unused
pub fn owner_entry_row(finding: &serde_json::Value) -> bool {
    finding["id"] == "SC4001"
        && finding["kind"] == "uncertifiable"
        && finding["blockedBy"]["kind"] == "owner-callers-unenumerated"
}

pub fn decode_findings(output: &[u8]) -> Vec<serde_json::Value> {
    let snapshot: serde_json::Value = serde_json::from_slice(output).expect("decode snapshot");
    snapshot["findings"]
        .as_array()
        .expect("snapshot findings")
        .clone()
}

pub fn temporary_directory(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = env::temp_dir().join(format!(
        "solid-checker-rust-{label}-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(&path).unwrap();
    path
}
