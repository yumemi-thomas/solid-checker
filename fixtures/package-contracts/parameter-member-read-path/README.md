# A parameter-member read is rooted at the whole chain, not its last segment

`parameter-member-read` pins *whether* a member read reaches a parameter. This
fixture pins *what path* the resulting read operation carries.

The read operation's input is `Parameter { index, path }`, and a consumer
matches that `path` as a **prefix** of the access it observes at runtime
(`type_facts::parameter_value_source_matches`). Two consequences drive every
case here:

- naming only the last segment of `options.source.slice(n)` publishes
  `["slice"]`, which asserts a `slice` property of `options` itself. The
  observed access starts with `source`, so the prefix never matches and the
  demand cannot be witnessed by any runtime — the claim is not merely imprecise,
  it is unprovable;
- a **shorter** path is a strictly weaker claim that every access rooted at the
  parameter satisfies. Truncating is therefore always sound, which is what makes
  the longest exact prefix the right answer for a segment that cannot be named.
  That holds against the exact matcher (`parameter_value_source_exact`) too,
  because both matchers compare a witness against the path the contract
  *states*, never against the access it was cut from — and because the walk
  refuses any chain whose root is not a plain identifier, so a stated path is
  always a true prefix of a real access rather than a path through some other
  value. `docs/precision-backlog.md` carries the full argument and the two
  premises it rests on.

The exports:

| export | access | path |
| --- | --- | --- |
| `oneSegment` | `options.slice(n)` | `["slice"]` |
| `twoSegments` | `options.source.slice(n)` | `["source", "slice"]` |
| `threeSegments` | `options.input.source.slice(n)` | `["input", "source", "slice"]` |
| `computedRoot` | `options[key].slice(n)` | `[]` |
| `computedInside` | `options.source[key].slice(n)` | `["source"]` |
| `disagreeingPaths` | `options.source…` and `options.other…` | unnamed |
| `readModuleLocal` | module-local value | no row |

`oneSegment` is the control that the deeper chains must not disturb: it is the
shape the generator always got right, and its path is byte-identical to what
`parameter-member-read`'s `drop` publishes.

`computedRoot` and `computedInside` are the fail-closed pair. Neither drops the
read row — dropping it would turn an unresolved access into the *negative*
claim "this export performs no parameter read" — and neither invents a segment.
They publish the exact prefix they proved and stop. A chain deeper than the
walk's 32-segment path limit is cut exactly the same way rather than dropped;
that case and the refused compound roots
(`(k ? options.a : options.b).c.slice(n)`, `(k, options.a).c…`,
`(options.a || fallback).c…`, `options().slice(n)`) are pinned by unit tests in
`rust/crates/solid-reactive-ir/src/indexes.rs`, since neither shape is a
realistic package to ship as a fixture.

`disagreeingPaths` pins that agreement is decided on whole paths. The model
publishes one row per parameter and names a path only when every contributing
access walks the same one; comparing last segments would have found `slice` on
both sides and published a single agreed path that neither access performs.

`readModuleLocal` is the negative control, mirroring `parameter-member-read`:
the identical chain on a module-created value publishes **no read row**. That is
the absence of a claim, not a certified negative: its summary is the bare
`{"call": {}, "shape": "callable"}` — every claim domain `Unknown`. It is what a
package must answer, because it cannot make a claim about a consumer's values
from its own.
