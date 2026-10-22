//! A visible `flush` that may run from an owned scope withholds the
//! ownerless-apply proof (ADR 0362).

use super::*;

impl SemanticLookup<'_> {
    /// ADR 0362: the queue a `flush` drains runs each `createEffect` apply under
    /// whatever owner is current where `flush` was called
    /// (`@solidjs/signals@2.0.0-rc.17` `dist/dev.js:1990-2024`, `runEffect`,
    /// raises the marker but never resets the context; `dist/dev-shared.js:2129`,
    /// `:2646`, `flush` does not either). A call that may run `flush` from an
    /// owned scope therefore breaks "the apply has no owner", which SC4001 and
    /// SC4005 both state. Until a flush site is joined to the effects it drains,
    /// any such call in the program withholds those proofs.
    ///
    /// A call counts when it is
    /// - an exact call of a primitive the dialect says may flush on its caller's
    ///   stack ([`solid_dialect::Dialect::may_flush_on_callers_stack`]), or
    /// - a call spelled `flush` whose callee does not resolve exactly
    ///   (counterevidence may be spelled; exact resolution governs proofs, not
    ///   withholdings).
    ///
    /// It is exempt where the ambient owner is provably none: a statement at
    /// module level (no function around it, no class instance field), or a call
    /// on a `createEffect` apply's stack ([`crate::effect_phase::Stack::Ownerless`]),
    /// where SC2008 places its own `flush`. A call anywhere else (a root body, a
    /// component, a helper, a handler) counts.
    pub(crate) fn flush_may_run_applies_owned(&self) -> bool {
        *self.flush_may_run_applies_owned.get_or_init(|| {
            self.files().iter().any(|file| {
                file.ast
                    .calls
                    .iter()
                    .any(|call| self.call_may_flush_from_owned_scope(file, call))
            })
        })
    }

    fn call_may_flush_from_owned_scope(
        &self,
        file: &FileFacts,
        call: &solid_facts::ast::CallFact,
    ) -> bool {
        let primitive = self.primitive_at_call(file, call.span);
        let counts = match primitive {
            Some(primitive) => self.dialect.may_flush_on_callers_stack(primitive),
            None => {
                file.source_text(call.callee).is_some_and(spelled_flush)
                    && !self.callee_resolves_exactly(file, call)
            }
        };
        if !counts || call.construct {
            return false;
        }
        // An action wrapper flushes at each invocation, not at the call that
        // builds it, so the call's position places nothing (no exemption).
        let flush_here =
            primitive.is_none_or(|primitive| self.dialect.flush_runs_during_call(primitive));
        if !flush_here {
            return true;
        }
        if module_level(&file.ast, call.span) {
            return false;
        }
        // The apply exemption needs the call's own declaration (the queue is
        // that package's) and an unresolved callee has none.
        let in_apply = primitive.is_some()
            && self
                .resolved_callee_call(file, call.callee)
                .and_then(|resolved| resolved.declaration.as_ref())
                .and_then(|declaration| {
                    crate::effect_phase::package_root(declaration.location.path.as_ref())
                })
                .and_then(|root| {
                    crate::effect_phase::effect_phase_origin(
                        file,
                        call.span,
                        &root,
                        &self.package_instances,
                        self,
                        crate::effect_phase::Stack::Ownerless,
                    )
                })
                .is_some();
        !in_apply
    }

    /// The callee resolves to a declaration through a valid resolved call.
    fn callee_resolves_exactly(&self, file: &FileFacts, call: &solid_facts::ast::CallFact) -> bool {
        self.resolved_callee_call(file, call.callee)
            .is_some_and(|resolved| {
                resolved.validity == ResolvedCallValidity::Valid && resolved.declaration.is_some()
            })
    }
}

/// The callee's last name is `flush` (`flush()`, `Solid.flush()`, `x?.flush()`).
fn spelled_flush(callee: &str) -> bool {
    callee
        .trim()
        .rsplit('.')
        .next()
        .is_some_and(|name| name.trim() == "flush")
}

/// No function (body or parameter list) and no instance field initializer
/// surrounds the span, so it runs while its module evaluates.
fn module_level(ast: &solid_facts::ast::AstFacts, span: Span) -> bool {
    !ast.functions
        .iter()
        .any(|function| function.span.contains(span))
        && !crate::execution_role::inside_instance_field_in(ast, span)
}

#[cfg(test)]
mod tests {
    use super::{module_level, spelled_flush};

    /// The span of the first call in `source` and whether it is module level.
    fn first_call_is_module_level(source: &str) -> bool {
        let ast = solid_facts::ast::extract("a.ts", source).expect("source parses");
        let call = ast.calls.first().expect("one call");
        module_level(&ast, call.span)
    }

    #[test]
    fn a_callee_spelled_flush_ends_in_that_name() {
        for callee in ["flush", " flush ", "Solid.flush", "a.b.flush", "x?.flush"] {
            assert!(spelled_flush(callee), "{callee}");
        }
        for callee in ["flushAll", "doFlush", "flush.call", "render", "flush()"] {
            assert!(!spelled_flush(callee), "{callee}");
        }
    }

    #[test]
    fn a_statement_in_the_module_body_is_module_level() {
        assert!(first_call_is_module_level("flush();"));
        assert!(first_call_is_module_level("if (ready) { flush(); }"));
        assert!(first_call_is_module_level("for (const x of xs) flush();"));
    }

    #[test]
    fn a_call_inside_any_function_is_not_module_level() {
        for source in [
            "function f() { flush(); }",
            "const f = () => flush();",
            "(() => { flush(); })();",
            "createRoot(() => { flush(); });",
            "class A { m() { flush(); } }",
            "function f(x = flush()) {}",
            "class A { x = flush(); }",
        ] {
            let ast = solid_facts::ast::extract("a.ts", source).expect("source parses");
            let call = ast
                .calls
                .iter()
                .find(|call| {
                    &source[call.callee.start as usize..call.callee.end as usize] == "flush"
                })
                .expect("the flush call");
            assert!(!module_level(&ast, call.span), "{source}");
        }
    }
}
