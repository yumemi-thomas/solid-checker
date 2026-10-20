import { createSignal } from "solid-js";

// Review counterexamples to the first version of ADR 0319. Each callback
// runs in a component body, or may: every read stays uncertifiable.

// A recursive element moves the callback to another prop on each level
// (`b={props.a}`, `c={props.b}`); the third level calls it in its body.
type Renamed = { a?: () => void; b?: () => void; c?: () => void; depth: number };
function RenamingBranch(props: Renamed) {
  const run = props.c;
  if (run) run();
  return (
    <div onClick={props.a}>
      {props.depth > 0 && <RenamingBranch b={props.a} c={props.b} depth={props.depth - 1} />}
    </div>
  );
}
export function CyclePath() {
  const [count] = createSignal(0);
  return <RenamingBranch a={() => console.log(count())} depth={3} />;
}

// The same through two components rendering each other.
type Mutual = { a?: () => void; b?: () => void; depth: number };
function Ping(props: Mutual) {
  return <div onClick={props.a}>{props.depth > 0 && <Pong b={props.a} depth={props.depth - 1} />}</div>;
}
function Pong(props: Mutual) {
  const run = props.b;
  if (run) run();
  return <span>{props.depth > 0 && <Ping a={props.a} depth={props.depth - 1} />}</span>;
}
export function MutualCycle() {
  const [count] = createSignal(0);
  return <Ping a={() => console.log(count())} depth={3} />;
}

// A function expression nested in an object-literal method is not that
// method: the method runs on keydown, the registered function in `Registered`'s
// body.
const registry: Array<() => number> = [];
function register(entry: () => number) {
  registry.push(entry);
}
export function Registered() {
  let total = 0;
  for (const entry of registry) total += entry();
  return <span>{total}</span>;
}
function useRegisteringRow(read: () => number, opened: () => void) {
  return {
    onKeyDown() {
      register(function () {
        return read();
      });
      opened();
    },
  };
}
export function NestedInMethod() {
  const [count] = createSignal(0);
  const [open, setOpen] = createSignal(false);
  const row = useRegisteringRow(
    () => count(),
    () => setOpen(true),
  );
  return <div {...row}>{open() ? <Registered /> : "closed"}</div>;
}

// Two returns hand back the callback in different literals; one gives `log`
// a function that calls `onClick` through `this`, and `log` runs in the body.
function logThis(this: { onClick?: () => void }) {
  this.onClick?.();
}
function pick(callback: () => void, flag: boolean) {
  if (flag) return { onClick: callback, log() {} };
  return { onClick: callback, log: logThis };
}
export function SeenOrigin() {
  const [count] = createSignal(0);
  const picked = pick(() => console.log(count()), Math.random() > 0.5);
  picked.log();
  return <button {...picked}>x</button>;
}

// The literal the handler object is spread into has a getter that calls the
// handler through `this` while the element reads its attributes.
function keyProps(onKey: () => void) {
  return { onKeyDown: () => onKey() };
}
export function SpreadThisGetter() {
  const [count] = createSignal(0);
  const attributes = {
    ...keyProps(() => console.log(count())),
    get title(): string {
      (this as { onKeyDown: () => void }).onKeyDown();
      return "t";
    },
  };
  return <div {...attributes}>a</div>;
}

// Round 2: property keys compare by their static name, however they are
// spelled. Each hook calls the callback in the caller's body through a key
// spelled differently from the member that reads it.
function useQuoted(callback: () => void) {
  const held = { "run": callback };
  const run = held.run;
  run();
  return { onClick: () => callback() };
}
export function QuotedKey() {
  const [count] = createSignal(0);
  const handlers = useQuoted(() => console.log(count()));
  return <button {...handlers}>q</button>;
}

function useSingleQuotedMember(callback: () => void) {
  const held = { 'run': callback };
  held['run']();
  return { onClick: () => callback() };
}
export function SingleQuotedMember() {
  const [count] = createSignal(0);
  const handlers = useSingleQuotedMember(() => console.log(count()));
  return <button {...handlers}>s</button>;
}

function useDoubleQuotedMember(callback: () => void) {
  const held = { run: callback };
  held["run"]();
  return { onClick: () => callback() };
}
export function DoubleQuotedMember() {
  const [count] = createSignal(0);
  const handlers = useDoubleQuotedMember(() => console.log(count()));
  return <button {...handlers}>d</button>;
}

function useNumericKey(callback: () => void) {
  const held = { 1: callback };
  held[1]();
  return { onClick: () => callback() };
}
export function NumericKey() {
  const [count] = createSignal(0);
  const handlers = useNumericKey(() => console.log(count()));
  return <button {...handlers}>n</button>;
}

function useTemplateKey(callback: () => void) {
  const held = { [`run`]: callback };
  held.run();
  return { onClick: () => callback() };
}
export function TemplateKey() {
  const [count] = createSignal(0);
  const handlers = useTemplateKey(() => console.log(count()));
  return <button {...handlers}>t</button>;
}

function useDestructuredQuoted(callback: () => void) {
  const held = { run: callback };
  const { "run": run } = held;
  run();
  return { onClick: () => callback() };
}
export function DestructuredQuoted() {
  const [count] = createSignal(0);
  const handlers = useDestructuredQuoted(() => console.log(count()));
  return <button {...handlers}>r</button>;
}

// A quoted later key replaces the spread member: `run` is `noop` when the
// body calls it, so the callback runs only on click (clean). Replacing
// another key leaves `run` the callback's caller (uncertifiable).
function baseHandlers(callback: () => void) {
  return { onClick: () => callback(), run: () => callback() };
}
const noop = () => undefined;
export function OverrideQuoted() {
  const [count] = createSignal(0);
  const merged = { ...baseHandlers(() => console.log(count())), "run": noop };
  merged.run();
  return <button {...merged}>o</button>;
}
export function OverrideOther() {
  const [count] = createSignal(0);
  const merged = { ...baseHandlers(() => console.log(count())), "other": noop };
  merged.run();
  return <button {...merged}>p</button>;
}

// Round 3: a non-computed `__proto__:` key (quoted or not) sets the
// prototype; it is no own property. `held.run` is `base.run`, run in the body.
type Run = { run: () => void };
export function ProtoAlias() {
  const [count] = createSignal(0);
  const base = { run: () => console.log(count()) };
  const held = { __proto__: base } as unknown as Run;
  const run = held.run;
  run();
  return <button onClick={() => base.run()}>p</button>;
}
export function QuotedProtoAlias() {
  const [count] = createSignal(0);
  const base = { run: () => console.log(count()) };
  const held = { "__proto__": base } as unknown as Run;
  const run = held.run;
  run();
  return <button onClick={() => base.run()}>q</button>;
}

// `super.fire()` in an object method runs the prototype's `fire` with the
// same `this`; a method mentioning `super` uses its receiver.
const firing = {
  fire(this: { onX: () => void }) {
    this.onX();
  },
};
function useSuper(callback: () => void) {
  const held = {
    __proto__: firing,
    onX: callback,
    m() {
      super.fire();
    },
  };
  held.m();
  return { onClick: () => held.onX() };
}
export function SuperCall() {
  const [count] = createSignal(0);
  const handlers = useSuper(() => console.log(count()));
  return <button {...handlers}>s</button>;
}
function useSuperSetPrototype(callback: () => void) {
  const held = {
    onX: callback,
    m() {
      super.fire();
    },
  };
  Object.setPrototypeOf(held, firing);
  held.m();
  return { onClick: () => held.onX() };
}
export function SuperSetPrototype() {
  const [count] = createSignal(0);
  const handlers = useSuperSetPrototype(() => console.log(count()));
  return <button {...handlers}>t</button>;
}
