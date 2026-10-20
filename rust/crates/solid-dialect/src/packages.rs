//! Package-name predicates over the carried dialects, and the host-target export conditions.

use super::*;

/// Whether `package` is a package whose own published bytes define some
/// dialect's primitives — an exact name match against
/// [`Dialect::primitive_defining_packages`] for either dialect.
///
/// Unioned across dialects deliberately: the question is about the *archive*
/// under analysis, not about which vocabulary a consuming project selected. A
/// 1.x project analyzing `@solidjs/signals` is analyzing bytes that define
/// 2.0's primitives, and the path heuristic that misreads them does not
/// consult the selected dialect either.
///
/// This is a **generation-scope** predicate, not a proof of identity: it
/// compares a name, with no version and no integrity behind it. It may only
/// ever *withhold* a claim (open a domain), never establish one — see ADR 0005.
#[must_use]
pub fn primitive_defining_package(package: &str) -> bool {
    if package.is_empty() {
        return false;
    }
    DIALECTS
        .iter()
        .any(|dialect| dialect.primitive_defining_packages().contains(&package))
}

/// Whether a *specifier* names the built-in runtime foundation — the package
/// itself or a subpath of it.
///
/// Built from [`Dialect::primitive_defining_packages`] rather than from a
/// list of its own: the three names were written out twice, and a fourth
/// core package added to a dialect would have reached one copy and not the
/// other.
#[must_use]
pub fn core_runtime_specifier(specifier: &str) -> bool {
    DIALECTS
        .iter()
        .flat_map(|dialect| dialect.primitive_defining_packages())
        .any(|name| {
            specifier == *name
                || specifier
                    .strip_prefix(name)
                    .is_some_and(|suffix| suffix.starts_with('/'))
        })
}

/// Whether a package reference must be kept out of the package boundary
/// because it names the built-in runtime foundation. This is not a resolver
/// or proof of runtime identity: even an untrusted spelling can only remove
/// a boundary obligation here, never grant built-in semantics.
#[must_use]
pub fn core_runtime_reference(package: &str, specifier: &str) -> bool {
    primitive_defining_package(package) || core_runtime_specifier(specifier)
}

/// A host-target export condition: the runtime a package is built for and a
/// consumer runs in.
///
/// The four are the mutually exclusive host axis of package export
/// conditions. A claim carrying one of these is about that host's runtime
/// bodies only.
///
/// An enum rather than a string so that adding a host is a compile error at
/// every match.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum HostTargetCondition {
    Browser,
    Node,
    Deno,
    Worker,
}

impl HostTargetCondition {
    /// Every host-target condition.
    pub const ALL: [Self; 4] = [Self::Browser, Self::Node, Self::Deno, Self::Worker];

    /// The export-condition spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Browser => "browser",
            Self::Node => "node",
            Self::Deno => "deno",
            Self::Worker => "worker",
        }
    }

    /// The host-target condition spelled `condition`, if it is one.
    #[must_use]
    pub fn from_condition(condition: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|host| host.as_str() == condition)
    }

    /// Whether `condition` is the spelling of some host-target condition. A
    /// consumer host that declared no conditions never receives an artifact
    /// case carrying one.
    #[must_use]
    pub fn names(condition: &str) -> bool {
        Self::from_condition(condition).is_some()
    }
}
