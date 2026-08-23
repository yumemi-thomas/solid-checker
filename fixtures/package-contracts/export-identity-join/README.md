# A declaration joins to its exports by identity, never by name text

Attribution's first rung used to return the local name whenever
`exports.contains_key(local_name)` — a name-text match with no identity check
at all. It is wrong in both directions, and this fixture pins both:

- **Too wide.** `internal.js` declares a private `Render`, and the entrypoint
  separately exports an unrelated `Render(input: number)`. The unresolved
  `.getThing` dispatch lives in the private one, and the text join wrote the
  unknown claim onto the public one. `UseChannel`, the export that actually
  reaches the obligation, was left certified — so the contract was wrong about
  two exports at once.
- **Too narrow.** `export { Panel, Panel as Root }` publishes one declaration
  under two names. The text join returned the first name it matched and
  stopped, so `Root` — the same function, the same obligation — was published
  as certified.

Both are joined through the Type Facts runtime identity and the canonical
symbol instead. `Panel` and `Root` resolve to the same identity and are marked
together; `internal.js`'s `Render` resolves to no entrypoint export, so the
obligation travels the call graph to `UseChannel`.

`Render` and `Isolated` are the negatives. Neither can reach the obligation,
and both must stay certified — a regression that answers "not an export" with
"undecidable" would widen to all five exports and be caught here.

The absence of a name-text fallback is deliberate. It survives only for the
whole-project mode with no entry file, where `exports` is keyed by the
project-wide export name and no identity channel exists at all; there the name
*is* the key.
