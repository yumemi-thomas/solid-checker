# A contract that does not describe the export a consumer uses

`partial-contract-package` ships a reviewed `solid-reactivity.json` that
describes `describedValue` and says nothing about `undescribedValue`. Both are
imported here; each is forwarded by one export.

## Every domain, for that export only

`forwardUndescribed` keeps all five claim domains unknown, and that is correct:
a contract with no summary for the export behind the call establishes nothing
at all -- not its reads, not its return, not whether it invokes the callback,
not its owner requirements, not its timing. This is the one obligation class
for which the whole-summary collapse was never a bug.

`forwardDescribed` publishes its complete proven summary. The two exports sit
in the same file, forward the same callback shape, and differ only in which
dependency export they call.

## No name-text widening

The obligation is filed at the *import binding* in `index.js`, which no
function body contains, so the lexical ladder cannot answer. Attribution
resolves it through the exact Type Facts symbol at that binding: every
reference to that symbol, and the exported function each reference sits in.
`forwardDescribed` references a different symbol and is untouched.

The rung this fixture removed was a scan of every call in the project whose
callee *text* equalled the missing export's name, or ended in `.` plus that
name. A local helper named `undescribedValue`, or any object with an
`.undescribedValue` member, would have pulled its enclosing export into the
unknown set on a string match. The exact symbol answers here without it, and
`identity-widening` is what the review plan records.
