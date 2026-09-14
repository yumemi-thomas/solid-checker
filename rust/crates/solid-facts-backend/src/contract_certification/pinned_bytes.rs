//! Digests of pinned, process-immutable files, taken once per process and
//! re-asserted by inode fingerprint on every later read.
//!
//! The probe harness re-asserts the pinned Node executable, the verifier image
//! and the Type Facts image between every worker launch (`watch_digests`), and
//! the Type Facts side re-hashes the shared producer image before every
//! launch. Hashing 117 MB of Node bytes on each of the thousands of censuses a
//! large certification takes was the single largest CPU cost of a corpus row
//! (122 s of 230 s census CPU on `solid-js@1.9.14`), and every one of those
//! reads returned the same digest.
//!
//! What this keeps: a file whose bytes change is still noticed. The digest is
//! served from the memo only while the file's device, inode, size, mtime and
//! ctime are the ones the bytes were hashed under, and an unprivileged writer
//! cannot rewrite bytes in place without moving ctime, nor replace the file
//! without changing its inode. The fingerprint and the hashed bytes are read
//! from the same open descriptor, so a path swapped between the two is not a
//! window either. What this gives up: a writer that can also set ctime — root,
//! or a clock it controls — could substitute bytes without a re-hash. The
//! pinned files are already outside every private tree and the worker runs
//! without that privilege, so the residual is the host's, not the probe's.

use std::{
    collections::BTreeMap,
    fs::File,
    io::{self, Read as _},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock, PoisonError},
};

use sha2::{Digest as _, Sha256};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Fingerprint {
    device: u64,
    inode: u64,
    length: u64,
    modified: (i64, i64),
    changed: (i64, i64),
}

#[cfg(unix)]
fn fingerprint_of(metadata: &std::fs::Metadata) -> Fingerprint {
    use std::os::unix::fs::MetadataExt as _;
    Fingerprint {
        device: metadata.dev(),
        inode: metadata.ino(),
        length: metadata.len(),
        modified: (metadata.mtime(), metadata.mtime_nsec()),
        changed: (metadata.ctime(), metadata.ctime_nsec()),
    }
}

#[cfg(not(unix))]
fn fingerprint_of(metadata: &std::fs::Metadata) -> Fingerprint {
    let stamp = |time: io::Result<std::time::SystemTime>| {
        time.ok()
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or((0, 0), |elapsed| {
                (
                    i64::try_from(elapsed.as_secs()).unwrap_or(i64::MAX),
                    i64::from(elapsed.subsec_nanos()),
                )
            })
    };
    Fingerprint {
        device: 0,
        inode: 0,
        length: metadata.len(),
        modified: stamp(metadata.modified()),
        changed: stamp(metadata.created()),
    }
}

fn memo() -> &'static Mutex<BTreeMap<PathBuf, (Fingerprint, String)>> {
    static MEMO: OnceLock<Mutex<BTreeMap<PathBuf, (Fingerprint, String)>>> = OnceLock::new();
    MEMO.get_or_init(|| Mutex::new(BTreeMap::new()))
}

/// The `sha256:<hex>` digest of the regular file at `path`, hashed on the
/// first call for this fingerprint and served from the memo while the
/// fingerprint holds. Any error is the caller's to classify; nothing is memoized
/// on an error.
pub(super) fn fingerprinted_digest(path: &Path) -> io::Result<String> {
    let mut file = File::open(path)?;
    let metadata = file.metadata()?;
    fingerprinted_digest_of_open(path, &mut file, &metadata)
}

/// [`fingerprinted_digest`] for a caller that already holds the file's
/// `lstat` — a tree walk does one per entry — so a memo hit costs no further
/// syscall. A tree census of thousands of files is bound by the kernel's
/// metadata path under concurrent walkers, not by hashing, and `open` +
/// `fstat` + `close` per file was three of its five syscalls.
pub(super) fn fingerprinted_digest_with_metadata(
    path: &Path,
    metadata: &std::fs::Metadata,
) -> io::Result<String> {
    if !metadata.file_type().is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "a fingerprinted file must be a regular file",
        ));
    }
    let fingerprint = fingerprint_of(metadata);
    if let Some((remembered, digest)) = memo()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(path)
        && *remembered == fingerprint
    {
        return Ok(digest.clone());
    }
    let mut file = File::open(path)?;
    let opened = file.metadata()?;
    fingerprinted_digest_of_open(path, &mut file, &opened)
}

fn fingerprinted_digest_of_open(
    path: &Path,
    file: &mut File,
    metadata: &std::fs::Metadata,
) -> io::Result<String> {
    if !metadata.file_type().is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "a pinned image must be a regular file",
        ));
    }
    let fingerprint = fingerprint_of(metadata);
    if let Some((remembered, digest)) = memo()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(path)
        && *remembered == fingerprint
    {
        return Ok(digest.clone());
    }
    let mut hash = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hash.update(&buffer[..read]);
    }
    let digest = format!("sha256:{:x}", hash.finalize());
    // The fingerprint was taken before the bytes were read. If the file moved
    // on underneath the read, the next call sees a new fingerprint and hashes
    // again; what must never happen is a memo entry whose fingerprint is
    // newer than the bytes it describes, so it is re-read here and the entry
    // is dropped when the two disagree.
    let after = fingerprint_of(&file.metadata()?);
    let mut memo = memo().lock().unwrap_or_else(PoisonError::into_inner);
    if after == fingerprint {
        memo.insert(path.to_path_buf(), (fingerprint, digest.clone()));
    } else {
        memo.remove(path);
    }
    Ok(digest)
}

/// Drops every memoized digest and read under `directory`. Called when a
/// private workspace is removed, so the memo tracks live files only.
pub(super) fn forget_under(directory: &Path) {
    memo()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .retain(|path, _| !path.starts_with(directory));
    file_memo()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .retain(|path, _| !path.starts_with(directory));
}

/// A small immutable input read whole: its bytes and their digest.
#[derive(Debug)]
pub(super) struct FingerprintedFile {
    pub(super) bytes: Vec<u8>,
    pub(super) sha256: String,
}

type FileMemo = BTreeMap<PathBuf, (Fingerprint, Arc<FingerprintedFile>)>;

fn file_memo() -> &'static Mutex<FileMemo> {
    static MEMO: OnceLock<Mutex<FileMemo>> = OnceLock::new();
    MEMO.get_or_init(|| Mutex::new(BTreeMap::new()))
}

/// The bytes and `sha256:<hex>` digest of the regular, non-symlink file at
/// `path`, at most `limit` bytes, read and hashed once per fingerprint. The
/// probe recipe corpus is loaded once per graph node per gating pass — 616
/// nodes × 12 passes on `corvu@0.7.2` — and every load re-read and re-hashed
/// the same 276 modules; the fingerprint discipline is the one
/// [`fingerprinted_digest`] states above.
pub(super) fn fingerprinted_read(path: &Path, limit: usize) -> io::Result<Arc<FingerprintedFile>> {
    let link = std::fs::symlink_metadata(path)?;
    if link.file_type().is_symlink() || !link.file_type().is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{} must be a regular non-symlink file", path.display()),
        ));
    }
    let mut file = File::open(path)?;
    let metadata = file.metadata()?;
    if !metadata.file_type().is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{} must be a regular non-symlink file", path.display()),
        ));
    }
    if usize::try_from(metadata.len()).map_or(true, |length| length > limit) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{} exceeds the {limit}-byte probe input limit",
                path.display()
            ),
        ));
    }
    let fingerprint = fingerprint_of(&metadata);
    if let Some((remembered, file)) = file_memo()
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(path)
        && *remembered == fingerprint
    {
        return Ok(Arc::clone(file));
    }
    let mut bytes = Vec::with_capacity(usize::try_from(metadata.len()).unwrap_or(0));
    file.read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{} exceeds the {limit}-byte probe input limit",
                path.display()
            ),
        ));
    }
    let read = Arc::new(FingerprintedFile {
        sha256: format!("sha256:{:x}", Sha256::digest(&bytes)),
        bytes,
    });
    let after = fingerprint_of(&file.metadata()?);
    let mut memo = file_memo().lock().unwrap_or_else(PoisonError::into_inner);
    if after == fingerprint {
        memo.insert(path.to_path_buf(), (fingerprint, Arc::clone(&read)));
    } else {
        memo.remove(path);
    }
    Ok(read)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest_of(bytes: &[u8]) -> String {
        format!("sha256:{:x}", Sha256::digest(bytes))
    }

    #[test]
    fn a_rewritten_file_is_hashed_again_and_a_stable_one_is_served_from_the_memo() {
        let directory = std::env::temp_dir().join(format!(
            "solid-checker-pinned-bytes-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |elapsed| elapsed.as_nanos())
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("image");
        std::fs::write(&path, b"first bytes").unwrap();
        assert_eq!(
            fingerprinted_digest(&path).unwrap(),
            digest_of(b"first bytes")
        );
        assert_eq!(
            fingerprinted_digest(&path).unwrap(),
            digest_of(b"first bytes")
        );
        assert!(memo().lock().unwrap().contains_key(&path));

        // Same length, different bytes: the size does not move, ctime does.
        std::fs::write(&path, b"other bytes").unwrap();
        assert_eq!(
            fingerprinted_digest(&path).unwrap(),
            digest_of(b"other bytes")
        );

        // A replacement file under the same path is a new inode.
        let staged = directory.join("staged");
        std::fs::write(&staged, b"swapped").unwrap();
        std::fs::rename(&staged, &path).unwrap();
        assert_eq!(fingerprinted_digest(&path).unwrap(), digest_of(b"swapped"));

        std::fs::remove_dir_all(&directory).unwrap();
        assert!(fingerprinted_digest(&path).is_err());
        forget_under(&directory);
        assert!(
            !memo()
                .lock()
                .unwrap()
                .keys()
                .any(|key| key.starts_with(&directory))
        );
    }

    #[test]
    fn a_read_is_served_from_the_memo_until_the_bytes_move() {
        let directory = std::env::temp_dir().join(format!(
            "solid-checker-pinned-read-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |elapsed| elapsed.as_nanos())
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("recipe.mjs");
        std::fs::write(&path, b"export const a = 1;").unwrap();
        let first = fingerprinted_read(&path, 1024).unwrap();
        assert_eq!(first.bytes, b"export const a = 1;");
        assert_eq!(first.sha256, digest_of(b"export const a = 1;"));
        let again = fingerprinted_read(&path, 1024).unwrap();
        assert!(
            Arc::ptr_eq(&first, &again),
            "an unchanged file is the memoized read"
        );
        std::fs::write(&path, b"export const a = 2;").unwrap();
        let changed = fingerprinted_read(&path, 1024).unwrap();
        assert_eq!(changed.bytes, b"export const a = 2;");
        assert_eq!(
            fingerprinted_read(&path, 4).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        std::fs::remove_dir_all(&directory).unwrap();
    }

    #[test]
    fn a_directory_is_refused_rather_than_memoized() {
        let error = fingerprinted_digest(&std::env::temp_dir()).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    }
}
