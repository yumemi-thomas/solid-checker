// ADR 1071: the settled module is read only as `m.createPlain`: the load hands out `createPlain` (through the barrel's re-export), not
// `createLoader`, so the hook's callers are all in view (none) and its `onSettled` needs no owner proof.
export function loadPlain() {
  return import("./barrel").then((m) => m.createPlain());
}
