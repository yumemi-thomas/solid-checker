# leaf-scope-optional-dom

**Claim (ADR 0318).** An optional call (`el?.m()`) whose exact declaration is
a `lib.dom` member in the audited script-free table is read like its
non-optional form (`cleanup::script_free_dom_member`): its arguments by the
standard-library timing table, and nothing else run. Every other optional DOM
member stays open, because it can run author code with no callback argument at
the site: focus/blur and click dispatch events, attribute and tree mutations
run custom-element reactions, and WebGL `getContext` can fire
`webglcontextcreationerror`.

| Case | Finding | Why |
| --- | --- | --- |
| `OptionalQueries` | none | selector/tree queries, layout and attribute reads, `contains`, `toDataURL` |
| `OptionalListener` | none | registration runs nothing; the listener is deferred even though it would register a cleanup |
| `OptionalRemoveListener`, `DirectRemoveListener` | `SC9012` uncertifiable | no audited timing for `removeEventListener`'s callable argument, in either form |
| `OptionalFocus`, `OptionalClick`, `OptionalDispatch`, `OptionalSetAttribute`, `OptionalAppend`, `OptionalRemove`, `OptionalScroll`, `OptionalGetContext`, `OptionalSelect` | `SC9012` uncertifiable | not in the table |
| `DirectFocus` | none (unchanged) | the non-optional form keeps the standard-library trust, which does not model dispatch |

`solid-js.d.ts` and `tsconfig.json` are copied from `leaf-scope-host-callbacks`;
the DOM declarations are TypeScript's own `lib.dom.d.ts`. The source passes
`tsc --noEmit`.
