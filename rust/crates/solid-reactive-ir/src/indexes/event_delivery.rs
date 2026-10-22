//! Visible synchronous DOM dispatch withholds an ownerless event proof.

use super::*;

impl SemanticLookup<'_> {
    /// ADR 0356: DOM dispatch runs handlers on its caller's stack. Until the
    /// event target and delivery sites are joined, any visible synchronous
    /// dispatch withholds the ownerless proof for event-only invocations.
    /// This is counterevidence, not a guessed handler edge. A project method
    /// with the same spelling does not select either library declaration.
    pub(crate) fn synchronous_event_dispatch_present(&self) -> bool {
        *self.synchronous_event_dispatch.get_or_init(|| {
            self.files().iter().any(|file| {
                file.ast.calls.iter().any(|call| {
                    self.resolved_callee_call(file, call.callee)
                        .and_then(crate::runtime_semantics::standard_call_name)
                        .is_some_and(|name| {
                            matches!(name, "EventTarget.dispatchEvent" | "HTMLElement.click")
                        })
                })
            })
        })
    }
}
