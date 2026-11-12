# exported-specifier-open-read

**Claim (batch 11, R1 of the re-review of #221; ADR 0970, ADR 0934).** In an **open** program (`.solid-checker/runtime.json`) an exported function may be called by code the program does not hold, so the read walks make no "every caller is visible" claim for it: a read in it, reached only by a component's effect here, is an uncertifiable SC1001 row (an importer that calls it in a component's body reads untracked: rc.21 STRICT_READ_UNTRACKED). `module_exports` (`execution_role/write_roles.rs`) took an export statement that contains the function, or Type Facts' export flag, for an export. A **specifier** (`export { specifier }`, `export { renamed as other }`, an arrow bound to a `const` exported so) names a binding declared elsewhere in the file: the statement does not contain the function, so the helper was taken for unexported and the read certified legal. It asks `exported_as_binding` (the declaration an export specifier's local resolves to is the function's own declaration, or the declarator whose initializer is the function: N1 of the second re-review of #221, a function expression carries a name of its own that no export can name, so `named` and `third` were certified legal too) now, so the component stop of the owner emission (`entered_by_tags_only`, which asks `entered_by_unseen_code`) and every read, write and creation walk that asks it agree.

| helper | before | now |
|---|---|---|
| `declared`, `export function` | SC1001 uncertifiable | SC1001 uncertifiable |
| `specifier`, `export { specifier }` | silent | SC1001 uncertifiable |
| `renamed`, `export { renamed as other }` | silent | SC1001 uncertifiable |
| `arrowed`, a `const` arrow, `export { arrowed }` | silent | SC1001 uncertifiable |
| `named`, `const named = function named() {..}`, `export { named }` | silent | SC1001 uncertifiable |
| `third`, `const third = function inner() {..}`, `export { third }` | silent | SC1001 uncertifiable |
| `local`, not exported | silent | silent |

The rows of the effect itself (SC3001, SC4001, SC6002 at `createEffect`) are the owner-call universe's and stand on both builds. The same program in a closed boundary is certified on both (every caller is visible). The unseen importer cannot be written by the differential fuzzer (scripts/fuzz/rediscovery.json says so); the oracle case for the owner emission's form of the same hole is `fixtures/runtime-oracle/owner-emission-open-importer` (`external` in oracle.json).
