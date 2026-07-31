//! Placeholder: filled by the attributes rule port.

use super::UpstreamCompatContext;
use crate::StaticViolation;
use solid_facts::FileFacts;

pub(super) fn check_file(
    file: &FileFacts,
    context: &UpstreamCompatContext<'_>,
    violations: &mut Vec<StaticViolation>,
) {
    let _ = (file, context, violations);
}
