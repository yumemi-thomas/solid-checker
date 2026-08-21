//! Solid 1.x `v1/jsx-no-undef` — report a `use:` directive only when
//! Oxc's lexical binder explicitly proves that no value-space binding exists.
//!
//! TypeScript deliberately does not bind the local-name node in a namespaced
//! JSX attribute, so it cannot diagnose an unresolved `use:directive`.
//! The structural fact extractor runs Oxc's semantic binder over the same AST
//! and records the exact lexical declaration selected for each directive name.
//! Missing directive bindings are therefore explicit negative facts rather
//! than unresolved TypeScript entity lookups.

use solid_facts::FileFacts;

use super::UpstreamCompatContext;
use crate::{StaticViolation, location};

pub(super) fn check_file(
    file: &FileFacts,
    _context: &UpstreamCompatContext<'_>,
    violations: &mut Vec<StaticViolation>,
) {
    for element in &file.ast.jsx_elements {
        for attribute in &element.attributes {
            if attribute
                .namespace
                .and_then(|namespace| file.source_text(namespace))
                != Some("use")
                || attribute.directive_binding.is_some()
            {
                continue;
            }
            let name = file.source_text(attribute.local_name).unwrap_or_default();
            violations.push(StaticViolation {
                id: "SC8005".into(),
                rule: "jsx-no-undef".into(),
                message: format!("'{name}' is not defined."),
                hint: "Import or declare this custom directive — lexical scope contains no binding for the use: name.".into(),
                location: location(file.path.shared(), attribute.local_name),
                analysis_context: String::new(),
                fixes: vec![],
                uncertain: false,
            });
        }
    }
}
