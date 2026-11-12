import { createEffect, createSignal } from "solid-js";

// ADR 0970, R1 of the re-review of #221, found on the read walk. The program is OPEN (`.solid-checker/runtime.json`): an export may be called by code the program does not hold,
// so a read in an exported helper has no "every caller is visible" claim (ADR 0934). `module_exports` took an export statement that contains the function (or Type Facts'
// flag) for an export, and an export specifier (`export { specifier }`, `export { renamed as other }`) names a binding declared elsewhere: the helper was taken for unexported, and
// the read a component's effect reaches was certified legal. An importer that calls it in a component's body runs the read untracked (rc.21: STRICT_READ_UNTRACKED).
// Not read as such on the read walk's side: a closed program (every caller is visible): `owner-emission-layer-stops`.
const [count] = createSignal(0);

// Exported by its declaration: an obligation before and after.
export function declared() {
  return count();
}
// Exported by a specifier, renamed, and an arrow bound to a const: certified legal before, an obligation now.
function specifier() {
  return count();
}
function renamed() {
  return count();
}
const arrowed = () => count();
// Control. Not exported: its only caller is the effect, so the read is tracked and legal.
function local() {
  return count();
}

export function App() {
  createEffect(
    () => declared() + specifier() + renamed() + arrowed() + local(),
    () => {},
  );
  return <div />;
}
export { specifier, renamed as other, arrowed };
