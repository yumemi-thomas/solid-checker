# A context provider's children are owned by its memo (ADR 1155 (b))

The Opus review of ADR 1147 (N1, Chrome-confirmed on rc.21, patterns `ctxmemo/*`): a creation or a cleanup written in a context provider's child was a violation
of the code around the provider element (SC4001 in a timer or an effect's apply, SC3001 in a tracked effect, SC6002 on an apply's stack), but rc.21 evaluates the
children only inside the provider's `children()` memo, whose owner is that memo: Chrome reports at the provider's tag (its own creation) or the control, never at
the child. For an exact provider (`callback_return::exact_context_provider`, ADR 1147's test), the child spans are owner-providing regions of the owner analysis,
the leaf walk files no creation or cleanup there, and the effect-phase walk stops there (`callback_return::in_exact_provider_children`). Owner questions only.

- Positive (the child's row goes; the tag's stays): `TimeoutApp` (p14b), `ApplyApp` (p9), `TrackedApp` (p8), `RenderApplyApp` (p2, the package's component
  written in the project).
- Negative (rows stay): `FlushApp` (a flush in the child: a stack question), `HandedApp` (a cleanup in a function the child hands to a timer), `WrittenApp` (a
  written `let` tag: no exact provider).

Stub: `context-provider-children-owned`'s, plus `children` (simplified from the published rc.21 typings).
