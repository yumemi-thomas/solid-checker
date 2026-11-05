# loading-on-outside-hold

`SC5006` · **warning** by default · uncertifiable only (internal, ADR 0328)

A `<Loading on={...}>` boundary waits on an async source that the analysis
cannot prove only the boundary reads.

## What it does

Solid 2's dev build warns `LOADING_ON_OUTSIDE_HOLD` when a change re-arms a
`Loading` boundary through `on`, and a source the boundary now waits on is also
read, in the same flush, by a frame reader outside it (a render effect, or
another boundary's tree): the frame waits on that read, so the fallback can
never be seen (rc.17 `@solidjs/signals` `dist/dev.js`, `reportOutsideHold`).

The warning rests on runtime state the analysis does not model: which boundary
instance holds which reader, whether the source's pending flight started in
the same flush (`source._time === clock`), whether the outside reader is
hidden by another fallback, and whether `on` reads ahead (`latest()`, a
verdict lane). So this rule never claims a violation. It records an
obligation instead of a certified clean (ADR 0517) at the `on` attribute of a
boundary when:

- a component, a spread or a member tag is rendered under it (its reads are
  not visible here);
- a read under it is not proven; or
- an async source read under it is referenced anywhere outside it (an exact
  symbol reference: a read, an alias, a hand-off).

A boundary with no `on` (and no spread that could carry one) never re-arms
and is not considered. A boundary under which no async source is read waits
on nothing and stays clean.

## Fix

Move the outside read under the boundary so one hold owns the data, or read
`latest()` in `on`, which shows the fallback beside the held frame.

## Precision

Recall-only. A proven violation needs the boundary-instance, same-flight and
visibility proofs listed above (`docs/open-work.md`).

A call with arguments counts as a no-argument one does. A resolved target proves nothing
where no edge follows its body (a method, a slot), and a Get of a literal's accessor or a
source handed to a call under the boundary keeps the obligation, blocker
`function-run-unplaced` (ADR 0660).
