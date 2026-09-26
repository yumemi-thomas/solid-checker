//! Which `solid-js` 2.x releases this vocabulary was read on, and what it
//! answers differently for the one reviewed release that moved an answer.
//!
//! Detection selects the Solid 2 *language* from the installed major. This is
//! the second question, asked of the exact installed version string. The
//! classification, and why each row sits where it does:
//!
//! | installed `solid-js` | review | analysis |
//! | --- | --- | --- |
//! | `2.0.0-rc.3` | the audited release: every `AUDITED_ARCHIVES` rc.3 tuple, the tsc oracle's pin | proceeds, no notice |
//! | `2.0.0-rc.0` | the release most vocabulary citations were read on (`callback_owners`, `onSettled`, the store typings); ADR 0005 names it the audited runtime for ownership, and the rc.0/rc.3 bodies the dialect cites are byte-identical (`docs/precision-backlog.md`, "rc.0 and rc.3 are the same bytes where the dialect cites them") | proceeds, no notice |
//! | `2.0.0-rc.9` | reviewed with known gaps (the 2026-09-26 review) | proceeds under [`Solid2::RC9`], one uncertifiable notice |
//! | `2.0.0-experimental.x` | refused: the pre-beta experiment, a different runtime | refused like `SC9013` |
//! | anything else of major 2: `rc.1`, `rc.2`, `rc.4`-`rc.8`, `rc.10`+, betas, `2.0.0`, `2.x.y`, an alpha, a build-metadata or range spelling | not compared against this vocabulary | proceeds under the audited vocabulary, one uncertifiable notice |
//!
//! The unreviewed row is fail-visible rather than refused or silent, and on
//! purpose. Silent is what the review found wrong: rc.9 changed four answers
//! the vocabulary gives by name, and `--certify` reported `certified` for code
//! rc.9 breaks. Refused would turn away every current Solid 2 project for
//! differences that, on the one release measured, were three additive gaps and
//! one typing change. What a release between rc.3 and rc.9 declares is not
//! known: the `Store<T> = T` change is in the rc.7-era *source*, not in any
//! published rc.4-rc.8 bytes anyone here read, so those releases keep the
//! audited answers and the notice says they were not compared. The same holds
//! above rc.9, where nothing has been read at all.
//!
//! Matching is exact on the trimmed string. A range (`^2.0.0-rc.3`) or build
//! metadata (`2.0.0-rc.3+local`) is not the release that was read; it lands in
//! the unreviewed row, which still analyzes and only adds the notice.

use super::Solid2;
use crate::{RefusedRelease, ReleaseReview, ReviewedRelease};

/// Whether a `createStore` root's own properties are declared `readonly`.
///
/// The only answer two reviewed releases of this vocabulary disagree on:
/// `@solidjs/signals@2.0.0-rc.3` declares `Store<T> = Readonly<T>`
/// (`dist/types/store/store.d.ts:4`) and `2.0.0-rc.9` declares `Store<T> = T`
/// (the same line). `tsc --noEmit` over the real published typings, one source
/// file, `strict`: rc.3 reports `TS2540: Cannot assign to 'name' because it is
/// a read-only property` on `profile.name = "Grace"` and rc.9 reports nothing
/// (fixtures `store-root-write-rc3` and `store-root-write-rc9`). The runtime
/// drops the write outside a setter on both (the review's probe H, dev and
/// prod), so on rc.9 the write is this checker's and on rc.3 it is
/// TypeScript's.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) enum StoreRootTyping {
    /// `Store<T> = Readonly<T>`: a root property write is TS2540.
    #[default]
    Readonly,
    /// `Store<T> = T`: a root property write type-checks.
    Mutable,
}

impl Solid2 {
    /// The vocabulary as audited on `2.0.0-rc.3` (and read on `rc.0`). Every
    /// release this module does not name a variant for is analyzed under it.
    pub const AUDITED: Self = Self {
        store_root: StoreRootTyping::Readonly,
    };

    /// The vocabulary for `solid-js@2.0.0-rc.9`: the audited answers, except
    /// that a store root's own properties are writable as far as TypeScript is
    /// concerned (B1 in the review). The review's other three items are
    /// additive -- rc.3's own typings reject each rc.9 form -- so modelling them
    /// needs no variant, and until they land they are `RC9_REVIEW`'s known
    /// gaps.
    pub const RC9: Self = Self {
        store_root: StoreRootTyping::Mutable,
    };
}

/// The review of `solid-js@2.0.0-rc.9` against this vocabulary.
const RC9_REVIEW_DOCUMENT: &str =
    "docs/package-contract-v2/audits/2026-09-26-solid-2-rc9-vocabulary-review.md";

/// What the rc.9 review left open. B1 is not here: [`Solid2::RC9`] answers it.
/// Each entry leaves this list when the dialect models it.
static RC9_REVIEW: ReviewedRelease = ReviewedRelease {
    version: "2.0.0-rc.9",
    known_gaps: &[
        "`omit(props, predicate)`: the predicate runs on every read of the returned view, and \
         code inside it is not classified, so it is neither checked nor reported uncertifiable",
        "rc.9's `solid-js` typings re-export `createErrorBoundary`, `createLoadingBoundary`, \
         `createRevealOrder`, `sharedConfig` and `$DEVCOMP`, which its own declarations no longer \
         declare, so under `skipLibCheck` those primitives are not resolved and their bodies are \
         not analyzed as such",
        "no negative row is granted for `@solidjs/signals@2.0.0-rc.9`, so certification closes \
         fewer claim domains than on the audited release",
    ],
    review: RC9_REVIEW_DOCUMENT,
};

/// The pre-beta Solid 2 experiment. Measured on `2.0.0-experimental.1`
/// (corvu's install, the review's § 8).
static PRE_BETA_EXPERIMENT: RefusedRelease = RefusedRelease {
    line: "2.0.0-experimental.x",
    reason: "it is the pre-beta Solid 2 experiment, which runs @solidjs/signals 0.x rather than \
             the release-candidate runtime this vocabulary was read on: its createEffect takes an \
             error handler at argument 2 and its createMemo a seed value at argument 1, where the \
             vocabulary reads options, and its Suspense and ErrorBoundary are not the Loading and \
             Errored boundaries the rules recognise",
    review: RC9_REVIEW_DOCUMENT,
};

/// The classification in the module table.
pub(super) fn review(installed: &str) -> ReleaseReview {
    let installed = installed.trim();
    match installed {
        "2.0.0-rc.0" | "2.0.0-rc.3" => ReleaseReview::Audited,
        "2.0.0-rc.9" => ReleaseReview::ReviewedWithGaps(&RC9_REVIEW),
        _ => match installed.strip_prefix("2.0.0-") {
            Some(prerelease)
                if prerelease == "experimental" || prerelease.starts_with("experimental.") =>
            {
                ReleaseReview::Refused(&PRE_BETA_EXPERIMENT)
            }
            _ => ReleaseReview::Unreviewed,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Dialect;

    #[test]
    fn releases_classify_by_what_was_read_on_their_bytes() {
        for audited in ["2.0.0-rc.3", "2.0.0-rc.0", " 2.0.0-rc.3\n"] {
            assert_eq!(review(audited), ReleaseReview::Audited, "{audited:?}");
        }
        assert_eq!(
            review("2.0.0-rc.9"),
            ReleaseReview::ReviewedWithGaps(&RC9_REVIEW)
        );
        for refused in [
            "2.0.0-experimental.1",
            "2.0.0-experimental.0",
            "2.0.0-experimental",
        ] {
            assert_eq!(
                review(refused),
                ReleaseReview::Refused(&PRE_BETA_EXPERIMENT),
                "{refused:?}"
            );
        }
        // Everything else of major 2 is fail-visible: analyzed, with the
        // notice. That includes the releases on either side of rc.9, the
        // betas, the stable line, spellings that are not one exact release,
        // and an alpha, which nobody here has measured.
        for unreviewed in [
            "2.0.0-rc.1",
            "2.0.0-rc.2",
            "2.0.0-rc.4",
            "2.0.0-rc.6",
            "2.0.0-rc.8",
            "2.0.0-rc.10",
            "2.0.0-rc.90",
            "2.0.0-rc.3.1",
            "2.0.0-beta.19",
            "2.0.0-alpha.0",
            "2.0.0-next.1",
            "2.0.0",
            "2.0.1",
            "2.1.0-rc.3",
            "2.1.0-experimental.1",
            "^2.0.0-rc.3",
            "2.0.0-rc.3+local",
            "v2.0.0-rc.3",
        ] {
            assert_eq!(
                review(unreviewed),
                ReleaseReview::Unreviewed,
                "{unreviewed:?}"
            );
        }
    }

    #[test]
    fn the_trait_answers_the_same_table_for_every_variant() {
        for vocabulary in [Solid2::AUDITED, Solid2::RC9] {
            assert_eq!(
                vocabulary.review_release("2.0.0-rc.9"),
                ReleaseReview::ReviewedWithGaps(&RC9_REVIEW)
            );
            assert_eq!(
                vocabulary.review_release("2.0.0-rc.3"),
                ReleaseReview::Audited
            );
        }
    }

    /// B1: the one answer the rc.9 variant moves, and the only one.
    #[test]
    fn only_the_store_root_typing_differs_between_the_variants() {
        assert!(Solid2::AUDITED.store_root_properties_are_readonly());
        assert!(!Solid2::RC9.store_root_properties_are_readonly());
        // The value the engine holds by name is the audited vocabulary.
        assert!(Solid2.store_root_properties_are_readonly());
        assert!(Solid2::default().store_root_properties_are_readonly());
        assert!(Solid2::RC9.store_setter_callback_enables_proxy_writes());
    }

    #[test]
    fn every_review_names_a_document_that_exists_and_a_gap() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        for document in [RC9_REVIEW.review, PRE_BETA_EXPERIMENT.review] {
            assert!(root.join(document).is_file(), "{document} is missing");
        }
        assert!(
            !RC9_REVIEW.known_gaps.is_empty(),
            "a review with no open gap is an audit; say so with ReleaseReview::Audited"
        );
    }
}
