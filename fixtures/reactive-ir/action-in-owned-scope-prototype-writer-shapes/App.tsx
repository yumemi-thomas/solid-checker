import "fz-wgetproto";
import "fz-wglobalkey";
import "fz-wprotokey";
import "fz-wprotopattern";
import "fz-wctorlist";
import "fz-weventview";
import "fz-wbound";
import "fz-wvalue";
import "fz-wdirect";
import "fz-wown";
import "fz-wclear";


// ADR 1108 part 2: each call runs the method one writer shape installs.
export function GetPrototypeOfBuiltin() {
  new Int8Array(2).copyWithin(0, 1);
  return null;
}
export function GlobalByConstantKey() {
  void new Float32Array(1).lastIndexOf(0);
  return null;
}
export function PrototypeByConstantKey() {
  void new Float64Array(1).indexOf(0);
  return null;
}
export function DestructuredPrototype() {
  void new Uint8Array(1).includes(0);
  return null;
}
export function ConstructorList() {
  void new Int32Array(1).every(() => true);
  return null;
}
export function ThroughEventView() {
  void new Uint32Array(1).some(() => false);
  return null;
}
export function BoundBuiltin() {
  void new Uint8ClampedArray(2).subarray(1);
  return null;
}
export function PrototypeOfValue() {
  void new BigInt64Array(1).fill(0n);
  return null;
}

// Unchanged (ADR 1108 part 1): `Float32Array.prototype.reverse = …`.
export function WrittenDirectly() {
  new Float32Array(1).reverse();
  return null;
}

// Silent: a function's own prototype by a constant key, a clear writer's destructured prototype, a method no package writes, the call in a handler.
export function OwnPrototype() {
  void [1].map((n) => n);
  return null;
}
export function WrittenByClearPackage() {
  void new Uint16Array(1).at(0);
  return null;
}
export function Unwritten() {
  void new Int8Array(1).slice();
  return null;
}
export function CalledInHandler() {
  return <button onClick={() => new Int8Array(2).copyWithin(0, 1)}>copy</button>;
}
