//! Content digests of the files a retained answer's validity reads
//! (ADR 0400): the sources the inference closure lists, the manifests above
//! them, the imported packages' manifests, read and hashed on several
//! workers every request. Anything but a regular file (a symlink, a
//! directory, an absent path) is observed as the inference closure does.

use std::{fs, io, path::Path};

use sha2::{Digest, Sha256};

/// SHA-256 of the file's bytes.
pub(crate) fn content_hash(path: &Path) -> io::Result<[u8; 32]> {
    Ok(Sha256::digest(fs::read(path)?).into())
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
