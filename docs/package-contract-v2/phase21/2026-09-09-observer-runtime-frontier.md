# Intersection Observer callback authority

Rechecked against the retained projects and published catalogs named by
`2026-09-08-original-helper-full.json` while the inert-initialization full
measurement runs. This is diagnostic evidence, not new certification.

The accepted `@solid-primitives/intersection-observer@2.2.5|solid1|only`
root describes `makeIntersectionObserver` as
`{"call":{},"shape":"callable"}`. It supplies no accepted callback claim.
The Solid 2 `3.0.0-next.3` floor/head graph refuses argument binding for that
function; the retained audit includes `callback-0` operation obligations.
The implementation passes `onChange` as argument zero to
`new IntersectionObserver(onChange, options)` in both versions.

`rust/crates/solid-reactive-ir/src/runtime_semantics.rs` explicitly models
`IntersectionObserver.construct` argument zero as a deferred callback, gated
by the selected standard construct signature and construct call kind. That
explains an inference route, not independent certification authority. The
certifier reports no exact direct-call or resolved-argument flow for this
callback. A default-library declaration cannot establish the runtime identity
of a mutable global constructor; the analogous Proxy counterexample is
recorded in `2026-09-08-post-corvu-frontier.md`.

Consequently the Solid 1 receipt cannot recover the Solid 2 claim, even before
accounting for its different importer, package artifact and dependency graph.
Removing the callback claim would not prove it. A supported recovery needs
an exact runtime-library callback premise with consumer-bound applicability,
or another positive implementation proof that discharges the actual demand.
Neither was added in this investigation. The two complete-row opportunities
remain unmeasured and must not be counted as gains.
