import { For, createMemo, createSignal } from "solid-js";

// A reactive read written in a named local function that a component body
// hands by name to a standard-library carrier that runs it before returning
// (`[1].forEach(f)`), to a tagged template, to `Function.prototype.call` /
// `apply` or to `Reflect.apply` runs in the body's strict-read window. rc.17
// warns STRICT_READ_UNTRACKED (the body runs under `untrack(() => Comp(props),
// label)`, solid.dev.js:44-70; a synchronous callee inherits the window).
// Saturation probes f1 f3 f4 f7 f8 f9 f13 f14 f15 f16 f22 f23 f30 and the
// controls f6 f19 f25 f27 f29 f31 (2026-11-01).

// Module-level function handed by name in a component body (f30).
const [r0] = createSignal(1);
function f0() {
  r0();
}

// Violation (f1): a const arrow handed to `Array.prototype.forEach`.
export function ForEachNamed() {
  const [r1] = createSignal(1);
  const f = () => {
    r1();
  };
  [1].forEach(f);
  return <p />;
}

// Violation (f3): `Array.prototype.map`.
export function MapNamed() {
  const [r1] = createSignal(1);
  const f = () => {
    r1();
    return 1;
  };
  [1].map(f);
  return <p />;
}

// Violation (f4): a `Promise` executor runs before the constructor returns.
export function PromiseExecutorNamed() {
  const [r1] = createSignal(1);
  const f = (res: () => void) => {
    r1();
    res();
  };
  new Promise<void>(f);
  return <p />;
}

// Violation (f7): a tagged template's tag is called where it is written.
export function TaggedTemplate() {
  const [r1] = createSignal(1);
  const tag = (s: TemplateStringsArray) => {
    r1();
    return String(s);
  };
  tag`x`;
  return <p />;
}

// Violation (f8): `Function.prototype.call` runs its receiver.
export function FunctionCall() {
  const [r1] = createSignal(1);
  const f = () => {
    r1();
  };
  f.call(null);
  return <p />;
}

// Violation (f9): `Reflect.apply` runs its first argument.
export function ReflectApply() {
  const [r1] = createSignal(1);
  const f = () => {
    r1();
  };
  Reflect.apply(f, null, []);
  return <p />;
}

// Violation (f13): `JSON.parse` calls its reviver before returning.
export function JsonReviver() {
  const [r1] = createSignal(1);
  const f = (_k: string, v: unknown) => {
    r1();
    return v;
  };
  JSON.parse("{}", f);
  return <p />;
}

// Violation (f14): `Array.from`'s map function.
export function ArrayFromMapFn() {
  const [r1] = createSignal(1);
  const f = (x: number) => {
    r1();
    return x;
  };
  Array.from([1], f);
  return <p />;
}

// Uncertifiable (f15): a `defineProperty` getter is handed in an object
// literal slot and run by a member read nothing follows.
export function DefinePropertyGetter() {
  const [r1] = createSignal(1);
  const o: { v?: number } = {};
  const f = () => {
    return r1();
  };
  Object.defineProperty(o, "v", { get: f });
  const x = o.v;
  void x;
  return <p />;
}

// Violation (f16): `Array.prototype.sort`'s comparator.
export function SortNamed() {
  const [r1] = createSignal(1);
  const f = (_a: number, _b: number) => {
    r1();
    return 0;
  };
  [2, 1].sort(f);
  return <p />;
}

// Violation (f22): the carrier is written in a `<For>` row callback, which
// runs untracked (rc.17 warns STRICT_READ_UNTRACKED).
export function ForEachNamedInFor() {
  const [r1] = createSignal(1);
  const f = () => {
    r1();
  };
  return (
    <For each={[1]}>
      {() => {
        [1].forEach(f);
        return <li />;
      }}
    </For>
  );
}

// Violation (f23): a hoisted function declaration handed by name.
export function ForEachHoistedFn() {
  const [r1] = createSignal(1);
  function f() {
    r1();
  }
  [1].forEach(f);
  return <p />;
}

// Violation (f30): the module-level function handed by name in the body.
export function ForEachModuleFn() {
  [1].forEach(f0);
  return <p />;
}

// Violation: the module-level function run through `Function.prototype.call`
// in the body (no probe; the reachability edge for a receiver is the entity's).
export function FunctionCallModuleFn() {
  f0.call(null);
  return <p />;
}

// Control, violation (f6): a method of a literal called in the body.
export function ObjectMethod() {
  const [r1] = createSignal(1);
  const o = {
    m() {
      return r1();
    },
  };
  o.m();
  return <p />;
}

// Control, violation (f19): the literal handed to the same carrier.
export function ForEachLiteral() {
  const [r1] = createSignal(1);
  [1].forEach(() => {
    r1();
  });
  return <p />;
}

// Control, legal (f25): `Promise.then` runs its callback from a microtask, on
// a fresh stack, outside every strict-read window.
export function ThenNamed() {
  const [r1] = createSignal(1);
  const f = () => {
    r1();
  };
  Promise.resolve().then(f);
  return <p />;
}

// Control, legal (f27): the carrier is written in an event handler.
export function ForEachNamedHandler() {
  const [r1] = createSignal(1);
  const f = () => {
    r1();
  };
  return <button onClick={() => [1].forEach(f)} />;
}

// Control, uncertifiable (f29): a `const` copy handed on (ADR 0516).
export function ForEachAlias() {
  const [r1] = createSignal(1);
  const f = () => {
    r1();
  };
  const g = f;
  [1].forEach(g);
  return <p />;
}

// Control, legal (f31): the carrier is written in a memo's compute, where the
// read tracks.
export function ForEachNamedMemo() {
  const [r1] = createSignal(1);
  const f = () => {
    r1();
  };
  const m = createMemo(() => {
    [1].forEach(f);
    return 1;
  });
  void m;
  return <p />;
}

// Exact Function.prototype.apply receiver, including a namespace carrier.
export function FunctionApply() {
  const [r] = createSignal(1);
  const f = () => { r(); };
  f.apply(null, []);
  return <p />;
}

// A project member with the same spelling supplies no inline proof.
export function ShadowedCarrier() {
  const [r] = createSignal(1);
  const f = () => { r(); };
  const box = { forEach(_cb: () => void) {} };
  box.forEach(f);
  return <p />;
}

// Stored without a followed invocation: fail closed, never a violation.
export function StoredReader() {
  const [r] = createSignal(1);
  const f = () => { r(); };
  const box = { f };
  void box;
  return <p />;
}

// Legal f25-shaped project scheduler: source timing is not proved here.
export function WrappedThen() {
  const [r] = createSignal(1);
  const f = () => { r(); };
  const later = (cb: () => void) => Promise.resolve().then(cb);
  later(f);
  return <p />;
}

// A carrier inside an uncalled named closure inherits no lexical body run.
export function StoredCarrier() {
  const [r] = createSignal(1);
  const f = () => { r(); };
  const g = () => [1].forEach(f);
  void g;
  return <p />;
}

// Nested literals in handed functions also fail closed until attributed.
export function NestedHandedReader() {
  const [r] = createSignal(1);
  const f = () => { [1].forEach(() => r()); };
  [1].forEach(f);
  return <p />;
}
