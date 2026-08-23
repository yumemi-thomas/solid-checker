# Package variant-precedence consumer

`package.json#exports` is an ordered map resolved first-match-wins, and a
generated contract records that order as `precedence` on each variant. This
fixture pins what the consumer does with it end to end, where the unit tests
in rust/crates/solid-reactive-ir/src/contracts.rs only pin the selection
function.

Both exports carry the same two overlapping branches — `browser` and
`development`, both satisfied by the selected runtime in
`.solid-checker/runtime.json` — and differ only in `precedence`. Two named
branches with no recorded order are undetermined, so `precedence` is the only
thing that can decide either case:

- `openSelected` records `development` at 0 and `browser` at 1. The lowest
  precedence is unique, so the `development` branch resolves and its accessor
  return is real: `UntrackedRead` is a proven reactive read outside tracking
  (`SC1001`, naming `development counter`), and `TrackedRead` stays clean.
  Resolving the other branch would make this fixture silent; failing closed
  would move the finding to the import binding.
- `openAmbiguous` declares 3 on both. The minimum is not unique, so nothing in
  the contract says which branch the resolver reaches first and substituting
  either would be a guess. The import binding is uncertifiable (`SC9005`,
  environment-dependent) and the identical accessor read in `AmbiguousRead`
  is never reported as reactive.

The runtime selection is required: with no environment selected,
`matches_conditions` matches nothing and both exports fail closed, which would
make the resolved half untestable.

The declarations are exact for this fixture package; every finding depends on
the runtime contract, not on trusting the declaration as runtime evidence.
