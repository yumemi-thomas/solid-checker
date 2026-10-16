# forwarded-tracked-compute

**Claim (ADR 0200).** Consider a function literal passed to a project function
whose only use of that parameter is a direct call written in the tracked
compute of a computation it creates. That literal runs only inside that
tracked compute, so a read written directly in it is tracked, not an
untracked read in the caller's body
(`execution_role::forwarded_tracked_compute_role`).

The compute must be the whole argument, written in the wrapper's own body, at
a slot that tracks its callback's reads:

- a primitive's tracked callback (`createMemo`'s compute, here).

Package contracts no longer supply execution facts. A source-visible adapter
can instead compose the local and dependency-source summaries below.

| Case | Finding | Why |
| --- | --- | --- |
| `UsesTracked` | none | `tracked` invokes `read` only in `createMemo(() => read())` |
| `UsesTrackedAndEager` | `SC1001` violation | the wrapper also calls `read()` in its body, during the call |
| `UsesUntracked` | none | the call sits in an unlabelled `untrack` callback, which raises no strict-read warning (ADR 0218) |
| `UsesTrackedLater` | `SC1001` uncertifiable | an async wrapper |
| `UsesForwarded` | none | the parameter is handed on uncalled as the memo's compute (`createMemo(read)`, ADR 0218) |

The stubs are copied from `forwarded-event-prop`.

`QueryOptions.tsx` and the additional typed adapters in `wrappers.ts` pin the
options-callback shape seen in an application wrapper of
`@tanstack/solid-query@6.0.0-rc.3`. These are project-owned functions exercising
source analysis, not copies or models of that package. No package-name trust
or accepted contract is involved. The adapter has the inner helper's generic
signature via a type annotation and an `as typeof` assertion; those wrappers
must preserve the implementation's execution behavior:

| Adapter | Expected SC1001 at `open()` |
| --- | --- |
| `wrapped` | none: forwarded callback runs only in a tracked memo |
| `wrappedAndEager` | violation: also invoked during the component's untracked call |
| `wrappedSnapshot` | none: invocation explicitly clears strict-read reporting with `untrack` |
| `wrappedLater` | uncertifiable: a returned closure supplies no proven invocation time |
| `wrappedUnknown` | uncertifiable: a declaration supplies no callback execution evidence |

The armed `typed_options_wrappers_preserve_source_callback_execution` process
test asserts the exact read spans and finding kinds. The new sources were also
checked with `tsc --noEmit` against the real published Solid 2.0.0-rc.13 typings
in the audited oracle install, without the fixture's declaration stub. No
TypeScript diagnostic is duplicated.

`MemberContexts.tsx` and `member-helpers.ts` pin the corresponding source-visible
object-member callback case. A helper can invoke `options.read()` only in a
tracked memo or a deferred host callback; its caller does not acquire an
untracked dispatch obligation merely because the exact property function has
no summary node. The invocation's context is retained in the internal member
summary and its cache dependency state. This proves the context, not the
property's implementation. The caller-supplied reactive reads in `TrackedMember`
and `DeferredMember` are clean. `MixedMember`, `UnknownMember` and `DefaultMember`
retain both the dispatch and callback-read obligations: a tracked use cannot
override an eager, unresolved or default-initializer use. The mixed exported
helper also retains its open-boundary dispatch obligation. The armed process
test checks exact caller spans across files. Both files pass `tsc --noEmit`
against published RC.13 typings without this fixture's stub.
