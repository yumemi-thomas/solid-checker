use std::{fs, path::PathBuf, time::Duration};

use super::{content_hash_within, input_digests, memo};

fn scratch(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("sc-digests-{label}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();
    path.canonicalize().unwrap()
}

#[test]
fn a_quiet_file_is_remembered_and_a_write_with_restored_times_is_still_seen() {
    let root = scratch("restored");
    let file = root.join("package.json");
    fs::write(&file, b"{\"a\":1}").unwrap();
    let first = content_hash_within(&file, Duration::ZERO).unwrap();
    assert!(memo().lock().unwrap().contains_key(&file));
    assert_eq!(content_hash_within(&file, Duration::ZERO).unwrap(), first);
    // Same length, modification time put back: only the change time moved.
    let modified = fs::metadata(&file).unwrap().modified().unwrap();
    std::thread::sleep(Duration::from_millis(20));
    fs::write(&file, b"{\"a\":2}").unwrap();
    fs::File::options()
        .write(true)
        .open(&file)
        .unwrap()
        .set_modified(modified)
        .unwrap();
    let second = content_hash_within(&file, Duration::ZERO).unwrap();
    assert_ne!(second, first);
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn a_file_changed_inside_the_quiet_window_is_never_remembered() {
    let root = scratch("racy");
    let file = root.join("racy.json");
    fs::write(&file, b"one").unwrap();
    let first = content_hash_within(&file, Duration::from_secs(3600)).unwrap();
    assert!(!memo().lock().unwrap().contains_key(&file));
    fs::write(&file, b"two").unwrap();
    assert_ne!(
        content_hash_within(&file, Duration::from_secs(3600)).unwrap(),
        first
    );
    fs::remove_dir_all(&root).unwrap();
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
    // Twice: the second answer comes from the memo for the regular file.
    for _ in 0..2 {
        assert_eq!(
            input_digests(&refs),
            solid_facts_backend::inferred_host_input_digests(&refs)
        );
    }
    fs::remove_dir_all(&root).unwrap();
}
