//! Exact published rc.13 declaration identities for ADR 0266.
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
        _ => None,
    }
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
    }
}
