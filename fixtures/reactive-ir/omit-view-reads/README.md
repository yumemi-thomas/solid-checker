# `omit` views read through to their props (ADR 0323, review round 9)

`const o = omit(props, "y")` is a view over `props`. Before this fixture's
change the checker modelled `merge` views (a binding initialized by a merge was
a props source) but not `omit` views, so `o.x` was never reported while
`merge(props, ...).x` was.

Claim: an `omit` view's first argument is its source; a read of a key the view
keeps is a props read of that component, **always uncertifiable** (the
view's keys are not the parameter's, so the caller classification cannot prove
the key reactive); a key hidden by a string-literal argument, directly or
through an inner `omit` view, reads nothing. A predicate or any other
non-literal hidden-key argument hides nothing the checker can name, so the key
stays a read. A spread of a view is an obligation, never a violation: the keys
the view hides may be the only reactive ones (this also holds for `merge`
views).

`Write` is an uncertifiable `SC2003` (2026-10-19). `omit` of a proxy (a
`merge` or split props object) returns a proxy whose `set` is a no-op, so the
write is dropped (`@solidjs/signals@2.0.0-rc.13` `dist/prod/store/utils.js:850`,
`set: trueFn`); `omit` of the compiler's plain props object returns a plain
copy (`dist/dev.js:4573-4600`), where the write succeeds (runtime oracle case
`r9-d5-writeomit` reads the value back). Which one the props object is, is not
proven here. The type of an `omit` result is writable,
so `tsc` reports nothing on it; the whole tree passes `tsc --noEmit` against
the published rc.13 declarations (copied unchanged from
`standard-runtime-configuration`).
