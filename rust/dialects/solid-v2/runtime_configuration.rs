//! Exact published rc.13 declaration identities for ADRs 0266 and 0268.
//! Re-exports resolve through binder aliases; source spellings are not inputs.

use crate::RuntimeConfigurationApi;

pub(super) fn api(path: &str, start: u64, end: u64) -> Option<RuntimeConfigurationApi> {
    let normalized = path.replace('\\', "/");
    let (_, declaration) = normalized.rsplit_once("/node_modules/")?;
    match (declaration, start, end) {
        ("@solidjs/signals/dist/types/core/external.d.ts", 457, 477) => {
            Some(RuntimeConfigurationApi::ExternalSource)
        }
        ("solid-js/types/index.d.ts", 2531, 2534)
        | ("@solidjs/signals/dist/types/index.d.ts", 1148, 1151)
        | ("@solidjs/signals/dist/types/core/dev.d.ts", 22966, 22969) => {
            Some(RuntimeConfigurationApi::DevelopmentHooks)
        }
        ("solid-js/types/index.d.ts", 2480, 2487)
        | ("@solidjs/signals/dist/types/index.d.ts", 1010, 1017)
        | ("@solidjs/signals/dist/types/core/dev.d.ts", 22529, 22536) => {
            Some(RuntimeConfigurationApi::Observation)
        }
        ("solid-js/types/internal.d.ts", 7472, 7484)
        | ("solid-js/types/server/shared.d.ts", 4285, 4297) => {
            Some(RuntimeConfigurationApi::HydrationHost)
        }
        _ => None,
    }
}

/// Boolean data reads cannot transport the host; context may only be tested.
/// Callback/member objects and computed keys have no harmless-read authority.
pub(super) fn harmless_host_read(member: &str, truthiness_only: bool) -> bool {
    matches!(member, "hydrating" | "done") || (member == "context" && truthiness_only)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_exact_catalog_identities_are_positive() {
        assert_eq!(
            api("/p/node_modules/solid-js/types/index.d.ts", 2531, 2534),
            Some(RuntimeConfigurationApi::DevelopmentHooks)
        );
        for path in [
            "/p/node_modules/ordinary/types/index.d.ts",
            "/p/solid-js/types/index.d.ts",
            "/p/node_modules/solid-js/types/local.d.ts",
        ] {
            assert_eq!(api(path, 2531, 2534), None);
        }
        assert_eq!(api("/p/node_modules/solid-js/types/index.d.ts", 1, 4), None);
        assert_eq!(
            api(
                "C:\\p\\node_modules\\@solidjs\\signals\\dist\\types\\core\\external.d.ts",
                457,
                477
            ),
            Some(RuntimeConfigurationApi::ExternalSource)
        );
        for (file, start, end) in [("internal", 7472, 7484), ("server/shared", 4285, 4297)] {
            let path = format!("/p/node_modules/solid-js/types/{file}.d.ts");
            assert_eq!(
                api(&path, start, end),
                Some(RuntimeConfigurationApi::HydrationHost)
            );
            assert_eq!(api(&path, start + 1, end), None);
            assert_eq!(api(&path.replace("solid-js", "other"), start, end), None);
            assert_eq!(
                api(&path.replace('/', "\\"), start, end),
                Some(RuntimeConfigurationApi::HydrationHost)
            );
        }
        assert!(harmless_host_read("hydrating", false));
        assert!(harmless_host_read("done", false));
        assert!(harmless_host_read("context", true));
        assert!(!harmless_host_read("context", false));
        assert!(!harmless_host_read("load", true));
    }
}
