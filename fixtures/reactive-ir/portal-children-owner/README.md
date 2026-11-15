# portal-children-owner

**Claim (ADR 0992).** `@solidjs/web`'s `Portal` reads `props.children` only in `createMemo(() => [startMarker, props.children])`, under the owner `createOwner()` made (rc.21 `dist/web.dev.js:2389-2399`). The Solid 2 dialect says so (`Dialect::component_entry("Portal", Prop("children"))`: `ReadTrackedUnderOwner`, read on the rc.21 triple only), so a creation written directly in a `Portal`'s children is evaluated by that read under an owner: it is no getter that runs where an unknown reader calls it (ADR 0971's decline `getter-runs-where-read`), the same as in a dialect control-flow element's children. Restores `clean-portal-show` (fixtures/uncertifiable-repro).

| site | outcome | why |
|---|---|---|
| `<Portal><div>{text()}</div></Portal>` (`Tip`) | silent | the tracked `{text()}` render effect is created in the portal's memo |
| the same through an alias (`import { Portal as Overlay }`) and a namespace (`<Web.Portal>`) | silent | the tag resolves to the declaration (every declaration in `@solidjs/web`, named `Portal`), not to the spelling |
| `<Portal mount={createMemo(() => document.body)()}>` (`Mounted`) | SC3001 and SC6002 uncertifiable (`getter-runs-where-read`) | `mount` is no such slot: the render effect's apply reads it again under `untrack(mount)` (`:2402`), on an apply's stack |
| a project component named `Portal` (`Shadowed.tsx`) | the rows it had | its children are a getter its own render reads; a name proves nothing |

`node_modules/@solidjs/web/index.d.ts` carries `Portal` byte-faithful from `@solidjs/web@2.0.0-rc.21` `types/index.d.ts:70-72`. The program is closed (`.solid-checker/runtime.json`).
