use std::{
    env, fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

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
