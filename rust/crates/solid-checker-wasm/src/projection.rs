//! The owned check's project projection (P5 S3c, G3).
//!
//! Native `--project` is canonicalized by the host's `realpath(3)` before
//! anything reads it (`solid-facts-backend/src/main.rs`). The owned check does
//! the same through WASI, against the directory store's view: the root, plus
//! virtual ancestors that each list only the next component. A spelling that
//! passes through a host place outside that view (for example macOS `/tmp`,
//! a link to `/private/tmp`) is answered `NotFound` there although the host
//! has the file. Only then, and only for `NotFound`, the checker asks its host
//! one question through `solid_owned_host` (`lib.rs`, `mod owned`): what that
//! spelling names when its outside places are read from the host.
//!
//! The answer's meaning is the store's (`packages/wasm/owned-snapshot.mjs`,
//! `project`). It projects a spelling only when the host resolves it by
//! symbolic-link substitution alone into the directory root, every parent
//! listing below the root is captured successfully, and each component is an
//! exact entry of its parent's listing. The projected path is then native's
//! under the further assumption that the host applies no other name
//! canonicalization (case, Unicode normalization, firmlinks). Every other
//! outcome is an explicit refusal. None of them is equivalence.
//!
//! This module only decodes the answer and turns it into the request's
//! project path or an error. It checks the answer's structure, not whether a
//! path lies inside the root: the root is known to the store, which checks
//! that before it encodes an answer.

use std::{
    error::Error,
    io,
    path::{Path, PathBuf},
};

pub(crate) const UNAVAILABLE: u8 = 0x00;
pub(crate) const NOT_OUTSIDE: u8 = 0x01;
pub(crate) const PROJECTED: u8 = 0x02;
pub(crate) const OUTSIDE_ROOT: u8 = 0x03;
pub(crate) const NAME_VARIANT: u8 = 0x04;
pub(crate) const HOST_ERROR: u8 = 0x05;
pub(crate) const FAULTED: u8 = 0x06;
pub(crate) const LISTING_UNAVAILABLE: u8 = 0x07;

/// WASI errnos a host lookup may answer: ENOENT, ENOTDIR, EACCES, EPERM, and
/// ELOOP from the store's own substitution bound.
const HOST_ERRNOS: [u16; 5] = [44, 54, 2, 63, 32];
/// WASI errnos a parent listing may answer.
const LISTING_ERRNOS: [u16; 4] = [44, 54, 2, 63];

/// One decoded answer.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Projection {
    Unavailable,
    NotOutside,
    Projected(String),
    OutsideRoot { path: String, root: String },
    NameVariant(String),
    HostError(u16),
    Faulted,
    ListingUnavailable { errno: u16, directory: String },
}

/// An answer path: strict UTF-8, NUL-free, absolute, and clean -- `/`, or
/// components that are neither empty nor `.` nor `..`, with no trailing `/`.
fn answer_path(bytes: &[u8], what: &str) -> Result<String, String> {
    let text = std::str::from_utf8(bytes).map_err(|_| format!("{what} is not UTF-8"))?;
    if text.contains('\0') {
        return Err(format!("{what} contains NUL"));
    }
    if text == "/" {
        return Ok(text.to_owned());
    }
    let Some(rest) = text.strip_prefix('/') else {
        return Err(format!("{what} {text:?} is not absolute"));
    };
    if rest
        .split('/')
        .any(|component| component.is_empty() || component == "." || component == "..")
    {
        return Err(format!("{what} {text:?} is not a clean path"));
    }
    Ok(text.to_owned())
}

fn errno(bytes: &[u8], allowed: &[u16], what: &str) -> Result<u16, String> {
    let [low, high] = bytes else {
        return Err(format!("{what} is not two bytes"));
    };
    let value = u16::from_le_bytes([*low, *high]);
    if allowed.contains(&value) {
        Ok(value)
    } else {
        Err(format!("{what} {value} is not one the answer may carry"))
    }
}

/// Decodes one whole answer, every byte accounted for.
pub(crate) fn decode(bytes: &[u8]) -> Result<Projection, String> {
    let (&tag, payload) = bytes
        .split_first()
        .ok_or_else(|| "the answer is empty".to_owned())?;
    let empty = |answer: Projection| {
        if payload.is_empty() {
            Ok(answer)
        } else {
            Err(format!(
                "tag {tag} carries {} unexpected bytes",
                payload.len()
            ))
        }
    };
    match tag {
        UNAVAILABLE => empty(Projection::Unavailable),
        NOT_OUTSIDE => empty(Projection::NotOutside),
        FAULTED => empty(Projection::Faulted),
        PROJECTED => Ok(Projection::Projected(answer_path(
            payload,
            "the projected path",
        )?)),
        OUTSIDE_ROOT => {
            let (length, rest) = payload
                .split_first_chunk::<4>()
                .ok_or_else(|| "the outside-root answer has no path length".to_owned())?;
            let length = usize::try_from(u32::from_le_bytes(*length))
                .map_err(|_| "the path length does not fit")?;
            if length == 0 || length >= rest.len() {
                return Err(format!(
                    "the outside-root path length {length} does not partition {} bytes",
                    rest.len()
                ));
            }
            let (path, root) = rest.split_at(length);
            Ok(Projection::OutsideRoot {
                path: answer_path(path, "the outside-root path")?,
                root: answer_path(root, "the outside-root root")?,
            })
        }
        NAME_VARIANT => Ok(Projection::NameVariant(answer_path(
            payload,
            "the name-variant path",
        )?)),
        HOST_ERROR => Ok(Projection::HostError(errno(
            payload,
            &HOST_ERRNOS,
            "the host errno",
        )?)),
        LISTING_UNAVAILABLE => {
            let (code, directory) = payload
                .split_first_chunk::<2>()
                .ok_or_else(|| "the listing answer has no errno".to_owned())?;
            Ok(Projection::ListingUnavailable {
                errno: errno(code, &LISTING_ERRNOS, "the listing errno")?,
                directory: answer_path(directory, "the listed directory")?,
            })
        }
        other => Err(format!("unknown tag {other}")),
    }
}

/// The project path a request names, as native `--project` names it.
///
/// `canonicalize` is the reactor's own (WASI's, against the store's view);
/// `project` asks the host and is called at most once, only after
/// `canonicalize` failed with `NotFound`. The spelling is passed to it exactly
/// as the request has it: `..` in it is physical and is never cleaned here.
pub(crate) fn resolve(
    spelling: &str,
    canonicalize: impl Fn(&Path) -> io::Result<PathBuf>,
    project: impl FnOnce(&str) -> Result<Vec<u8>, String>,
) -> Result<String, Box<dyn Error>> {
    let path = Path::new(spelling);
    if !path.is_absolute() {
        return Err(format!("project {spelling} is not an absolute path").into());
    }
    let original = match canonicalize(path) {
        Ok(canonical) => return Ok(canonical.to_string_lossy().into_owned()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => error,
        Err(error) => return Err(error.into()),
    };
    let malformed = |detail: String| -> Box<dyn Error> {
        format!("the owned host's project projection is malformed: {detail}").into()
    };
    let answer = decode(&project(spelling).map_err(malformed)?).map_err(malformed)?;
    match answer {
        Projection::Unavailable | Projection::NotOutside => Err(original.into()),
        Projection::Projected(projected) => match canonicalize(Path::new(&projected)) {
            Ok(again) if again.to_str() == Some(projected.as_str()) => Ok(projected),
            Ok(again) => Err(format!(
                "the owned host projected {spelling} to {projected}, which canonicalizes to {}",
                again.display()
            )
            .into()),
            Err(error) => Err(format!("the owned host projected {spelling} to {projected}, which fails: {error}").into()),
        },
        Projection::OutsideRoot { path, root } => {
            Err(format!("project {spelling} resolves on the host to {path}, outside the directory root {root}").into())
        }
        Projection::NameVariant(path) => Err(format!(
            "project {spelling}: {path} is not spelled as its directory entry; name-insensitive aliases are not resolved"
        )
        .into()),
        Projection::HostError(errno) => Err(io::Error::from_raw_os_error(i32::from(errno)).into()),
        Projection::Faulted => Err("the owned checker's input faulted during project projection".into()),
        Projection::ListingUnavailable { errno, directory } => Err(format!(
            "project {spelling}: listing {directory} to confirm its spelling failed: {}; the spelling is unconfirmed, not absent",
            io::Error::from_raw_os_error(i32::from(errno))
        )
        .into()),
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;

    fn with_path(tag: u8, path: &str) -> Vec<u8> {
        let mut bytes = vec![tag];
        bytes.extend_from_slice(path.as_bytes());
        bytes
    }

    fn outside(path: &[u8], root: &[u8], length: u32) -> Vec<u8> {
        let mut bytes = vec![OUTSIDE_ROOT];
        bytes.extend_from_slice(&length.to_le_bytes());
        bytes.extend_from_slice(path);
        bytes.extend_from_slice(root);
        bytes
    }

    fn listing(errno: u16, directory: &str) -> Vec<u8> {
        let mut bytes = vec![LISTING_UNAVAILABLE];
        bytes.extend_from_slice(&errno.to_le_bytes());
        bytes.extend_from_slice(directory.as_bytes());
        bytes
    }

    #[test]
    fn every_tag_decodes_exactly() {
        assert_eq!(decode(&[UNAVAILABLE]), Ok(Projection::Unavailable));
        assert_eq!(decode(&[NOT_OUTSIDE]), Ok(Projection::NotOutside));
        assert_eq!(decode(&[FAULTED]), Ok(Projection::Faulted));
        assert_eq!(
            decode(&with_path(PROJECTED, "/r/p/tsconfig.json")),
            Ok(Projection::Projected("/r/p/tsconfig.json".into()))
        );
        assert_eq!(
            decode(&with_path(PROJECTED, "/")),
            Ok(Projection::Projected("/".into()))
        );
        assert_eq!(
            decode(&outside(b"/o/tsconfig.json", b"/r", 16)),
            Ok(Projection::OutsideRoot {
                path: "/o/tsconfig.json".into(),
                root: "/r".into()
            })
        );
        assert_eq!(
            decode(&with_path(NAME_VARIANT, "/r/ACTUAL")),
            Ok(Projection::NameVariant("/r/ACTUAL".into()))
        );
        for code in HOST_ERRNOS {
            let mut bytes = vec![HOST_ERROR];
            bytes.extend_from_slice(&code.to_le_bytes());
            assert_eq!(decode(&bytes), Ok(Projection::HostError(code)));
        }
        for code in LISTING_ERRNOS {
            assert_eq!(
                decode(&listing(code, "/r/sub")),
                Ok(Projection::ListingUnavailable {
                    errno: code,
                    directory: "/r/sub".into()
                })
            );
        }
        // A leading U+FEFF is part of a name, kept as the character it is.
        assert_eq!(
            decode(&with_path(PROJECTED, "/r/\u{feff}p")),
            Ok(Projection::Projected("/r/\u{feff}p".into()))
        );
    }

    #[test]
    fn every_malformed_answer_is_refused() {
        let mut cases: Vec<Vec<u8>> = vec![
            vec![],
            vec![0x08],
            vec![0xff, 1, 2],
            vec![UNAVAILABLE, 0],
            vec![NOT_OUTSIDE, b'/'],
            vec![FAULTED, 0, 0],
            vec![PROJECTED],
            vec![HOST_ERROR],
            vec![HOST_ERROR, 44],
            vec![HOST_ERROR, 44, 0, 0],
            vec![HOST_ERROR, 8, 0],
            vec![LISTING_UNAVAILABLE, 32, 0, b'/', b'r'],
            vec![LISTING_UNAVAILABLE, 2, 0],
            vec![LISTING_UNAVAILABLE, 2],
            listing(2, "r/sub"),
            outside(b"/o", b"/r", 0),
            outside(b"/o", b"/r", 3),
            outside(b"/o", b"/r", 4),
            outside(b"/o", b"", 2),
            vec![OUTSIDE_ROOT, 2, 0, 0],
            outside(b"/o/", b"/r", 3),
        ];
        for path in [
            "", "r/p", "/r/p/", "/r//p", "/r/./p", "/r/../p", "/r/p\0", "//",
        ] {
            cases.push(with_path(PROJECTED, path));
            cases.push(with_path(NAME_VARIANT, path));
        }
        cases.push(vec![PROJECTED, b'/', 0xff]);
        cases.push(outside(b"/o", &[b'/', 0xc3], 2));
        for bytes in cases {
            assert!(
                decode(&bytes).is_err(),
                "{bytes:?} decoded as {:?}",
                decode(&bytes)
            );
        }
    }

    fn not_found() -> io::Error {
        io::Error::from(io::ErrorKind::NotFound)
    }

    #[test]
    fn success_and_other_errors_never_ask_the_host() {
        let asked = Cell::new(false);
        let project = |_: &str| {
            asked.set(true);
            Ok(vec![UNAVAILABLE])
        };
        assert_eq!(
            resolve("/r/p", |_| Ok(PathBuf::from("/r/p")), project).unwrap(),
            "/r/p"
        );
        let project = |_: &str| {
            asked.set(true);
            Ok(vec![UNAVAILABLE])
        };
        let error = resolve(
            "/r/f/x",
            |_| Err(io::Error::from(io::ErrorKind::NotADirectory)),
            project,
        )
        .unwrap_err();
        assert_eq!(
            error.downcast_ref::<io::Error>().map(io::Error::kind),
            Some(io::ErrorKind::NotADirectory)
        );
        let project = |_: &str| {
            asked.set(true);
            Ok(vec![UNAVAILABLE])
        };
        assert_eq!(
            resolve("tsconfig.json", |_| Err(not_found()), project)
                .unwrap_err()
                .to_string(),
            "project tsconfig.json is not an absolute path"
        );
        assert!(!asked.get());
    }

    #[test]
    fn unavailable_and_not_outside_keep_the_original_error() {
        for tag in [UNAVAILABLE, NOT_OUTSIDE] {
            let asked = Cell::new(false);
            let original = || io::Error::new(io::ErrorKind::NotFound, "the original NotFound");
            let error = resolve(
                "/m/absent",
                |_| Err(original()),
                |_| {
                    asked.set(true);
                    Ok(vec![tag])
                },
            )
            .unwrap_err();
            assert!(asked.get());
            assert_eq!(error.to_string(), "the original NotFound");
            assert_eq!(
                error.downcast_ref::<io::Error>().map(io::Error::kind),
                Some(io::ErrorKind::NotFound)
            );
        }
    }

    #[test]
    fn the_spelling_reaches_the_host_unchanged() {
        let seen = std::cell::RefCell::new(String::new());
        let spelling = "/tmp/a/../b/./tsconfig.json";
        let _ = resolve(
            spelling,
            |_| Err(not_found()),
            |given| {
                seen.borrow_mut().push_str(given);
                Ok(vec![NOT_OUTSIDE])
            },
        );
        assert_eq!(seen.borrow().as_str(), spelling);
    }

    #[test]
    fn a_projection_is_used_only_if_it_canonicalizes_to_itself() {
        let canonicalize = |path: &Path| {
            if path == Path::new("/tmp/p/tsconfig.json") {
                Err(not_found())
            } else {
                Ok(path.to_path_buf())
            }
        };
        assert_eq!(
            resolve("/tmp/p/tsconfig.json", canonicalize, |_| Ok(with_path(
                PROJECTED,
                "/private/tmp/p/tsconfig.json"
            )))
            .unwrap(),
            "/private/tmp/p/tsconfig.json"
        );
        let differs = |path: &Path| {
            if path == Path::new("/a/t") {
                Err(not_found())
            } else {
                Ok(PathBuf::from("/r/OTHER"))
            }
        };
        assert_eq!(
            resolve("/a/t", differs, |_| Ok(with_path(PROJECTED, "/r/t")))
                .unwrap_err()
                .to_string(),
            "the owned host projected /a/t to /r/t, which canonicalizes to /r/OTHER"
        );
        let fails = |_: &Path| Err(not_found());
        let error = resolve("/a/t", fails, |_| Ok(with_path(PROJECTED, "/r/t")))
            .unwrap_err()
            .to_string();
        assert_eq!(
            error,
            format!(
                "the owned host projected /a/t to /r/t, which fails: {}",
                not_found()
            )
        );
    }

    #[test]
    fn every_refusal_names_its_cause() {
        let missing = |_: &Path| Err(not_found());
        let refusal = |bytes: Vec<u8>| resolve("/a/t", missing, move |_| Ok(bytes)).unwrap_err();
        assert_eq!(
            refusal(outside(b"/o/t", b"/r", 4)).to_string(),
            "project /a/t resolves on the host to /o/t, outside the directory root /r"
        );
        assert_eq!(
            refusal(with_path(NAME_VARIANT, "/r/ACTUAL")).to_string(),
            "project /a/t: /r/ACTUAL is not spelled as its directory entry; name-insensitive aliases are not resolved"
        );
        assert_eq!(
            refusal(vec![FAULTED]).to_string(),
            "the owned checker's input faulted during project projection"
        );
        for code in HOST_ERRNOS {
            let mut bytes = vec![HOST_ERROR];
            bytes.extend_from_slice(&code.to_le_bytes());
            let error = refusal(bytes);
            assert_eq!(
                error
                    .downcast_ref::<io::Error>()
                    .and_then(io::Error::raw_os_error),
                Some(i32::from(code))
            );
        }
        for code in LISTING_ERRNOS {
            let error = refusal(listing(code, "/r/sub"));
            assert!(error.downcast_ref::<io::Error>().is_none());
            assert_eq!(
                error.to_string(),
                format!(
                    "project /a/t: listing /r/sub to confirm its spelling failed: {}; the spelling is unconfirmed, not absent",
                    io::Error::from_raw_os_error(i32::from(code))
                )
            );
        }
    }

    #[test]
    fn a_protocol_failure_or_malformed_answer_is_never_the_original_error() {
        let missing = |_: &Path| {
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                "the original NotFound",
            ))
        };
        let error = resolve(
            "/a/t",
            missing,
            |_| Err("project answered status 21".into()),
        )
        .unwrap_err();
        assert_eq!(
            error.to_string(),
            "the owned host's project projection is malformed: project answered status 21"
        );
        assert!(error.downcast_ref::<io::Error>().is_none());
        let error = resolve("/a/t", missing, |_| Ok(vec![0x09])).unwrap_err();
        assert_eq!(
            error.to_string(),
            "the owned host's project projection is malformed: unknown tag 9"
        );
        let error = resolve("/a/t", missing, |_| Ok(vec![HOST_ERROR, 9, 0])).unwrap_err();
        assert!(
            error
                .to_string()
                .starts_with("the owned host's project projection is malformed: ")
        );
    }
}
