# A re-export across a `.d.ts` split keeps its walk verdicts (ADR 0142)

**The trap this fixture exists for: `helpers.d.ts` must stay beside
`helpers.js`.** Delete it and TypeScript resolves `./helpers.js` to the
implementation again, the import binding's symbol is the function's own, and
the fixture stops testing anything.

`index.js` writes `import { … } from "./helpers.js"` and re-exports the
bindings. TypeScript resolves that specifier to `helpers.d.ts`, so the
export's canonical symbol is the declaration file's, while every generator walk
that feeds a proposal -- the valueless-completion walk (ADR 0035), the value
completion walk (ADR 0113), the argument-container walk (ADR 0115), the
`creates` walk and the owner requirements -- indexed the function Node loads,
in `helpers.js`. Before ADR 0142 the two never met: `isEven`, `reset` and
`choose` proposed `callbacks` and `reads` (the IR's own summary, which already
follows ADR 0137's join) and nothing for `creates` or `returns`, although the
same bodies declared in `index.js` propose both.

The attach now reads the export's symbol through ADR 0137's exact
declaration-to-runtime redirects:

- `isEven` proposes one `plain` return, `reset` `returns: []`, `choose` a
  return of each of its caller's arguments `first` and `second`, and all three
  `creates: []` -- exactly what `isZero`, declared in the entry file, proposes;
- `isOdd` is re-exported by `export { isOdd } from "./helpers.js"`. The
  redirects join import bindings only, so it still proposes neither domain.
  It is the negative control, and the one remaining gap this fixture names.

Only proposals move. Every closure here is still decided by the certifier's
census over the authenticated implementation.
