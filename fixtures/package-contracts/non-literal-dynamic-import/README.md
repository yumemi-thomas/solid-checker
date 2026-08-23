# A non-literal `import()` blocks on the runtime claim, not on the record

**The trap this fixture exists for: the specifier in `load` must stay a template
with a substitution.** Make it a literal and both the walk and the compiler
resolve it, and there is nothing left to say.

`load` returns ``import(`./mod-${name}.js`)``. Nothing static can name what that
loads: the generator's walk cannot, and neither can the compiler — the pinned
producer emits no import fact for it at all. Before attestation the walk said
`closure could not be fully enumerated: a dynamic import() whose specifier is
not a string literal`, and that one sentence carried two different claims:

1. *the record is incomplete* — which attestation makes **false**. The record is
   the analyzing program's own file list; it names every byte the analysis read,
   and it reads `index.js` alone here because `mod-a.js` is never opened.
2. *the runtime may load a module the analysis never read* — which stays
   **true**, and which no module graph can settle.

So the note survives, with the second claim as its whole content, on
`runtimeNotes` rather than `notes`. `expected-generation.json` pins that split:
no `notes`, one `runtimeNotes` entry, and `index.js` as the only module.

**Why two fields rather than two spellings of one.** They block different gates,
and both blocks are deliberate:

- `contract verify` refuses the document for either (`collectBlockers` in
  packages/cli/scripts/contract-verification.mjs raises its own sentence for
  each). RFC 0002 §2 condition 4 is unchanged in effect: negative claims resting
  on a file set nothing bounds are not machine-verifiable.
- `contract review --transfer-from` blocks on `notes` and **not** on
  `runtimeNotes` (`closureDifference` in
  packages/cli/scripts/review-contract.mjs). Two generations whose attested
  records are byte-identical describe the same bytes; the runtime is no less
  unbounded in either, and refusing the transfer would be refusing it for a
  reason that did not change.

`mod-a.js` sits on disk deliberately: it must stay out of the record, because the
analysis never opened it. A reconciliation that recorded a file merely because it
exists would fail here.
