import { createSignal, createMemo, createContext, useContext, merge, omit } from "solid-js";

function handlers(cb: () => void) {
  return { onClick: () => cb() };
}

// 1. Hook result held in a memo; the memo's value is called in the body.
export function MemoHeld() {
  const [count] = createSignal(0);
  const h = createMemo(() => handlers(() => console.log(count())));
  h().onClick();
  return <button {...h()}>1</button>;
}

// 2. Props passed whole into a project function that calls the member.
function useProps(p: { onX: () => void }) {
  p.onX();
}
function Whole(props: { onX: () => void }) {
  useProps(props);
  return <button onClick={props.onX}>2</button>;
}
export function WholeProps() {
  const [count] = createSignal(0);
  return <Whole onX={() => console.log(count())} />;
}

// 3. merge / omit of props, then called in body.
function Merged(props: { onX: () => void }) {
  const m = merge({ onX: () => undefined }, props);
  m.onX();
  return <button onClick={props.onX}>3</button>;
}
export function MergeProps() {
  const [count] = createSignal(0);
  return <Merged onX={() => console.log(count())} />;
}
function Omitted(props: { onX: () => void; y?: number }) {
  const rest = omit(props, "y");
  rest.onX();
  return <button onClick={props.onX}>3b</button>;
}
export function OmitProps() {
  const [count] = createSignal(0);
  return <Omitted onX={() => console.log(count())} />;
}

// 4. Object.assign of the returned holder.
export function Assigned() {
  const [count] = createSignal(0);
  const h = handlers(() => console.log(count()));
  const copy = Object.assign({}, h);
  copy.onClick();
  return <button {...h}>4</button>;
}

// 5. Context provider value.
const Ctx = createContext<{ onX: () => void }>({ onX: () => undefined });
function Consumer() {
  const c = useContext(Ctx);
  c.onX();
  return <span>c</span>;
}
export function ViaContext() {
  const [count] = createSignal(0);
  const value = { onX: () => console.log(count()) };
  return (
    <Ctx value={value}>
      <button onClick={value.onX}>5</button>
      <Consumer />
    </Ctx>
  );
}

// 6. Bound handler array returned in a holder; nested array.
function bound(cb: (d: number) => void) {
  return { onClick: [cb, 1] as [(d: number) => void, number] };
}
export function BoundArray() {
  const [count] = createSignal(0);
  const h = bound((d) => console.log(d, count()));
  h.onClick[0](1);
  return <button {...h}>6</button>;
}

// 7. Tuple hook.
function tuple(cb: () => void) {
  return [() => cb(), 1] as const;
}
export function Tuple() {
  const [count] = createSignal(0);
  const t = tuple(() => console.log(count()));
  t[0]();
  return <button onClick={t[0]}>7</button>;
}

// 8. then() of returned value.
function retain(cb: () => void) {
  return () => cb();
}
export function Then() {
  const [count] = createSignal(0);
  const run = retain(() => console.log(count()));
  Promise.resolve().then(run);
  return <button onClick={run}>8</button>;
}

// 9. Getter on the options object that calls the callback.
function useOpts(o: { onX: () => void; readonly label: string }) {
  const l = o.label;
  return { onClick: () => o.onX(), l };
}
export function GetterOpts() {
  const [count] = createSignal(0);
  const opts = {
    onX: () => console.log(count()),
    get label() {
      opts.onX();
      return "x";
    },
  };
  const h = useOpts(opts);
  return <button onClick={h.onClick}>9</button>;
}

// 10. Class instance holding the callback.
class Holder {
  constructor(readonly cb: () => void) {}
  run() {
    this.cb();
  }
}
export function ClassHolder() {
  const [count] = createSignal(0);
  const k = new Holder(() => console.log(count()));
  k.run();
  return <button onClick={() => k.run()}>10</button>;
}
