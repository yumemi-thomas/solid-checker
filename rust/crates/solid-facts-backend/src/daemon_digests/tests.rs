use std::{fs, path::PathBuf};

use super::{content_hash, input_digests};

fn scratch(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("sc-digests-{label}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();
    path.canonicalize().unwrap()
}

#[test]
fn input_digests_are_the_inference_closures_for_every_kind_of_path() {
    let root = scratch("kinds");
    let file = root.join("file.ts");
    fs::write(&file, b"export {};").unwrap();
    fs::create_dir_all(root.join("dir")).unwrap();
    fs::write(root.join("dir/inside.ts"), b"x").unwrap();
    std::os::unix::fs::symlink(&file, root.join("link.ts")).unwrap();
    std::os::unix::fs::symlink(root.join("nowhere"), root.join("dangling.ts")).unwrap();
    let paths = [
        file.clone(),
        root.join("dir"),
        root.join("link.ts"),
        root.join("dangling.ts"),
        root.join("absent.ts"),
        file.clone(),
    ];
    let refs = paths.iter().map(PathBuf::as_path).collect::<Vec<_>>();
    assert_eq!(
        input_digests(&refs),
        solid_facts_backend::inferred_host_input_digests(&refs)
    );
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn a_kept_digest_follows_every_rewrite_of_its_file() {
    use sha2::Digest as _;
    let root = scratch("kept");
    let file = root.join("package.json");
    let expect = |bytes: &[u8]| -> [u8; 32] { sha2::Sha256::digest(bytes).into() };
    fs::write(&file, b"{\"a\":1}").unwrap();
    assert_eq!(content_hash(&file).unwrap(), expect(b"{\"a\":1}"));
    assert_eq!(content_hash(&file).unwrap(), expect(b"{\"a\":1}"));
    // The same length: the change time moves the stamp.
    fs::write(&file, b"{\"a\":2}").unwrap();
    assert_eq!(content_hash(&file).unwrap(), expect(b"{\"a\":2}"));
    // Replaced by another inode.
    let other = root.join("next.json");
    fs::write(&other, b"{\"b\":3}").unwrap();
    fs::rename(&other, &file).unwrap();
    assert_eq!(content_hash(&file).unwrap(), expect(b"{\"b\":3}"));
    fs::remove_file(&file).unwrap();
    assert!(content_hash(&file).is_err());
    fs::remove_dir_all(&root).unwrap();
}
