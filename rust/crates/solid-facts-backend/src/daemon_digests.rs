//! Content digests of the files a retained answer's validity reads, kept
//! while the file's metadata stamp holds (ADR 0400).
//!
//! Every request used to re-read and re-hash each of these files: the sources
//! the inference closure lists, the manifests above them, the imported
//! packages' manifests. A project's tree is thousands of them and one edit
//! changes one. A digest is reused when the file's stamp -- device, inode,
//! length, modification and change time, the rule ADR 0382 already holds the
//! tracked sources to -- is the one it was read under, and only when the file
//! was quiet long enough that a later write cannot share its stamp (the
//! "racy" rule: a file changed within [`QUIET`] of the read is read again
//! next time). Anything but a regular file (a symlink, a directory, an
//! absent path) is observed afresh every time, as before.

use std::{
    collections::HashMap,
    fs, io,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    sync::Mutex,
    time::{Duration, SystemTime},
};

use sha2::{Digest, Sha256};

/// How long before a read a file must have last changed for its digest to be
/// kept. Longer than any filesystem timestamp granularity this runs on (HFS+
/// and ext3 keep whole seconds).
const QUIET: Duration = Duration::from_secs(2);

type Stamp = (u64, u64, u64, i64, i64, i64, i64);

fn stamp_of(metadata: &fs::Metadata) -> Stamp {
    (
        metadata.dev(),
        metadata.ino(),
        metadata.len(),
        metadata.mtime(),
        metadata.mtime_nsec(),
        metadata.ctime(),
        metadata.ctime_nsec(),
    )
}

/// [`QUIET`], or in a debug build what `SOLID_CHECKER_DIGEST_QUIET_MS` says,
/// so a process test can reuse a digest without waiting seconds.
fn quiet_window() -> Duration {
    #[cfg(debug_assertions)]
    if let Some(millis) = std::env::var("SOLID_CHECKER_DIGEST_QUIET_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
    {
        return Duration::from_millis(millis);
    }
    QUIET
}

/// Whether nothing wrote this file within `window` of `now`.
fn quiet(metadata: &fs::Metadata, now: SystemTime, window: Duration) -> bool {
    let at = |seconds: i64, nanoseconds: i64| {
        let nanoseconds = u32::try_from(nanoseconds).unwrap_or(0);
        u64::try_from(seconds)
            .ok()
            .map(|seconds| SystemTime::UNIX_EPOCH + Duration::new(seconds, nanoseconds))
    };
    let (Some(modified), Some(changed)) = (
        at(metadata.mtime(), metadata.mtime_nsec()),
        at(metadata.ctime(), metadata.ctime_nsec()),
    ) else {
        return false;
    };
    now.duration_since(modified.max(changed))
        .is_ok_and(|age| age >= window)
}

type Kept = HashMap<PathBuf, (Stamp, [u8; 32])>;

fn memo() -> &'static Mutex<Kept> {
    static KNOWN: std::sync::OnceLock<Mutex<Kept>> = std::sync::OnceLock::new();
    KNOWN.get_or_init(Mutex::default)
}

/// SHA-256 of the file's bytes, from the memo when its stamp still holds.
pub(crate) fn content_hash(path: &Path) -> io::Result<[u8; 32]> {
    content_hash_within(path, quiet_window())
}

fn content_hash_within(path: &Path, window: Duration) -> io::Result<[u8; 32]> {
    let before = fs::symlink_metadata(path)?;
    if !before.is_file() {
        return Ok(Sha256::digest(fs::read(path)?).into());
    }
    let stamp = stamp_of(&before);
    if let Ok(known) = memo().lock()
        && let Some((recorded, hash)) = known.get(path)
        && *recorded == stamp
    {
        return Ok(*hash);
    }
    let now = SystemTime::now();
    let hash: [u8; 32] = Sha256::digest(fs::read(path)?).into();
    // Kept only if the file did not move while it was read, and is quiet.
    if quiet(&before, now, window)
        && fs::symlink_metadata(path).is_ok_and(|after| stamp_of(&after) == stamp)
        && let Ok(mut known) = memo().lock()
    {
        known.insert(path.to_path_buf(), (stamp, hash));
    }
    Ok(hash)
}

/// `solid_facts_backend::inferred_host_input_digests`, with a regular file's
/// digest taken from [`content_hash`]: the identity of such a file is
/// `sha256:<hex of its bytes>`, and its digest that string's.
pub(crate) fn input_digests(paths: &[&Path]) -> Vec<Option<[u8; 32]>> {
    let regular = parallel_map(paths, |path| {
        fs::symlink_metadata(path)
            .ok()
            .is_some_and(|metadata| metadata.is_file())
    });
    let mut digests = vec![None; paths.len()];
    let files = (0..paths.len())
        .filter(|&index| regular[index])
        .collect::<Vec<_>>();
    let hashed = parallel_map(&files, |&index| content_hash(paths[index]).ok());
    let mut other = Vec::new();
    let mut hashed = hashed.into_iter();
    for (index, is_file) in regular.iter().enumerate() {
        if *is_file {
            match hashed.next().flatten() {
                Some(hash) => {
                    digests[index] = Some(
                        Sha256::digest(format!("sha256:{:x}", hash_hex(&hash)).as_bytes()).into(),
                    );
                }
                // Unreadable, or it stopped being a regular file meanwhile.
                None => other.push(index),
            }
        } else {
            other.push(index);
        }
    }
    let rest = other.iter().map(|&index| paths[index]).collect::<Vec<_>>();
    for (index, digest) in other
        .iter()
        .zip(solid_facts_backend::inferred_host_input_digests(&rest))
    {
        digests[*index] = digest;
    }
    digests
}

struct Hex<'a>(&'a [u8; 32]);

impl std::fmt::LowerHex for Hex<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0
            .iter()
            .try_for_each(|byte| write!(formatter, "{byte:02x}"))
    }
}

fn hash_hex(hash: &[u8; 32]) -> Hex<'_> {
    Hex(hash)
}

/// `map` over `items`, on several workers when there are many. The results
/// are in `items` order. The reads this runs are independent and
/// filesystem-bound; six workers, as `inferred_host::inputs::identities`
/// found best on APFS.
pub(crate) fn parallel_map<T: std::marker::Sync, R: Send>(
    items: &[T],
    map: impl Fn(&T) -> R + std::marker::Sync,
) -> Vec<R> {
    const WORKERS: usize = 6;
    const PARALLEL_FROM: usize = 256;
    if items.len() < PARALLEL_FROM {
        return items.iter().map(map).collect();
    }
    let chunk = items.len().div_ceil(WORKERS);
    std::thread::scope(|scope| {
        let map = &map;
        let workers = items
            .chunks(chunk)
            .map(|part| scope.spawn(move || part.iter().map(map).collect::<Vec<_>>()))
            .collect::<Vec<_>>();
        workers
            .into_iter()
            .flat_map(|worker| worker.join().expect("a filesystem worker panicked"))
            .collect()
    })
}

#[cfg(test)]
mod tests;
