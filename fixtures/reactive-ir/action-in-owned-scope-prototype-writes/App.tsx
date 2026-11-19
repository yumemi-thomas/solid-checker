import "fz-proto";
import "fz-protoalias";
import "fz-protodefine";
import "fz-protoclear";

// ADR 1108: a method the standard library declares, which an installed package replaces on a built-in's prototype where it loads: every object of that built-in
// inherits the replacement, so the call runs the package's code whatever type the library declares the method with.
export function PatchedByAssignment() {
  new Float32Array(1).reverse(); // `Float32Array.prototype.reverse = …`
  return null;
}
export function PatchedThroughAlias() {
  new Uint8Array(2).fill(1); // `const UP = Uint8Array.prototype; UP.fill = …`
  return null;
}
export function PatchedByDefine() {
  void new Int16Array(2).join("-"); // `Object.defineProperty(Int16Array.prototype, "join", …)`
  return null;
}

// Unchanged (ADR 0974): a global the package writes by name.
export function GlobalWrittenByName() {
  void atob("eA==");
  return null;
}

// Silent: the writer is clear (its closure names no Solid package), the method is one no package writes, or the call runs where no owner is active.
export function PatchedByClearPackage() {
  [2, 1].sort(); // `fz-protoclear` writes `Array.prototype.sort` and cannot call an action
  return null;
}
export function Unpatched() {
  void [1, 2].map((n) => n * 2);
  void "x".trim();
  void new Float32Array(1).slice();
  return null;
}
export function PatchedInHandler() {
  return <button onClick={() => new Float32Array(1).reverse()}>reverse</button>;
}
