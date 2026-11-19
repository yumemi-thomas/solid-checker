import "fz-psites";
import "fz-psitesclear";

interface Reversible {
  reverse(): unknown;
}

// ADR 1108 part 2: each call may run a method `fz-psites` writes on a built-in's prototype.
export function CalledThroughCall() {
  const a = new Float32Array(1);
  a.reverse.call(a);
  return null;
}
export function CalledThroughApply() {
  const a = new Float32Array(1);
  a.reverse.apply(a, []);
  return null;
}
export function CalledThroughReflect() {
  const a = new Float32Array(1);
  Reflect.apply(a.reverse, a, []);
  return null;
}
export function CalledDestructured() {
  const { reverse } = new Float32Array(1);
  reverse();
  return null;
}
export function CalledOnInterface() {
  const a: Reversible = new Float32Array(1);
  a.reverse();
  return null;
}
export function CalledOnAny() {
  const a: any = new Float32Array(1);
  a.reverse();
  return null;
}
export function BareInherited() {
  dispatchEvent(new Event("fzprobe"));
  return null;
}
export function TestRunsExec() {
  void /fzprobe/.test("x");
  return null;
}
export function MapRunsSet() {
  void new Map([["fzprobe", 1]]);
  return null;
}
export function ToStringRunsJoin() {
  void ["fzprobe"].toString();
  return null;
}
export function TemplateRunsJoin() {
  void `${["fzprobe"]}`;
  return null;
}
export function StringifyRunsToJson() {
  void JSON.stringify({ at: new Date(0) });
  return null;
}
export function AccessorRead() {
  void new DataView(new ArrayBuffer(1)).byteLength;
  return null;
}

// Unchanged (ADR 1108 part 1): a direct call of the library's method.
export function CalledDirectly() {
  new Float32Array(1).reverse();
  return null;
}

// Silent: a method only a clear package writes, methods no package writes, and the call in a handler.
export function WrittenByClearPackage() {
  new Uint16Array(1).fill(0);
  return null;
}
export function Unwritten() {
  void [1, 2].map((n) => n * 2);
  void "x".trim();
  void new Float32Array(1).slice();
  return null;
}
export function CalledInHandler() {
  return <button onClick={() => { const a = new Float32Array(1); a.reverse.call(a); }}>reverse</button>;
}
