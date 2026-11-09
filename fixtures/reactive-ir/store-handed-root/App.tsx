import { createContext, createEffect, createRoot, createStore, merge, omit, useContext } from "solid-js";
import type { StoreSetter } from "solid-js";
import { useVals } from "./hooks";
import { shared, other } from "./shared";
import * as stores from "./shared";

type V = { v: number };
type Props = { s: V; setS: StoreSetter<V> };

// Violation: every site passes the store `s1` (props member).
function PropMember(props: Props) {
  createRoot(() => {
    props.s.v = 1;
  });
  return <div />;
}

// Violation: a slot of the destructured props parameter.
function PropSlot({ s }: Props) {
  createRoot(() => {
    s.v = 2;
  });
  return <div />;
}

// Violation: `merge` view, the props object last.
function PropMerge(props: Props) {
  const merged = merge({ extra: 1 }, props);
  createRoot(() => {
    merged.s.v = 3;
  });
  return <div />;
}

// Violation: `omit` view of another key.
function PropOmit(props: Props & { extra: number }) {
  const rest = omit(props, "extra");
  createRoot(() => {
    rest.s.v = 4;
  });
  return <div />;
}

// Violation: the component's only site spreads its parent's own props.
function Spread(props: Props) {
  createRoot(() => {
    props.s.v = 5;
  });
  return <div />;
}
function Forward(props: Props) {
  return <Spread {...props} />;
}

// Violation: a write in a named closure called once, the store a hook slot.
function HookSlot() {
  const { s } = useVals();
  const wrap = () => {
    s.v = 6;
  };
  wrap();
  return <div />;
}

// Violation: a member of a hook result, written in a timer.
function HookMember() {
  const vals = useVals();
  setTimeout(() => {
    vals.s.v = 7;
  }, 0);
  return <div />;
}

const Ctx = createContext<Props>();

// Violation: a destructured context value, every provider passing the store.
function ContextSlot() {
  const { s } = useContext(Ctx);
  createEffect(
    () => 1,
    () => {
      s.v = 8;
    },
  );
  return <div />;
}

// Violation: an imported store through a `const` alias.
function Imported() {
  const alias = shared;
  createRoot(() => {
    alias.v = 9;
  });
  return <div />;
}

// Silent: every site passes a closed plain literal.
function PlainProp(props: { box: V }) {
  createRoot(() => {
    props.box.v = 10;
  });
  return <div />;
}

// Silent: the handed setter's callback writes its draft.
function Draft(props: Props) {
  createRoot(() => {
    props.setS((draft) => {
      draft.v = 11;
    });
  });
  return <div />;
}

// Uncertain: one site passes the store, another a plain literal.
function Mixed(props: { box: V }) {
  createRoot(() => {
    props.box.v = 12;
  });
  return <div />;
}

// Uncertain: inside the handed setter's own callback (a window).
function InSetter(props: Props) {
  createRoot(() => {
    props.setS(() => {
      props.s.v = 13;
    });
  });
  return <div />;
}

// Uncertain: a link past the store (ADR 0465).
function Nested(props: { deep: { a: V } }) {
  createRoot(() => {
    props.deep.a.v = 14;
  });
  return <div />;
}

// Uncertain: a callback a project function hands on, which a setter may run.
const queue: Array<() => void> = [];
function later(fn: () => void) {
  queue.push(fn);
}
function HandedOn(props: Props) {
  later(() => {
    props.s.v = 15;
  });
  return <div />;
}

// Uncertain: the slot `t` is replaced somewhere, so it is not the value passed.
function Replaced(props: { t: V }) {
  createRoot(() => {
    props.t.v = 16;
  });
  return <div />;
}
const holder: { t: V } = { t: { v: 0 } };
holder.t = { v: 1 };

// Uncertain: an exported component, callers unseen.
export function Exported(props: Props) {
  createRoot(() => {
    props.s.v = 17;
  });
  return <div />;
}

// Violation, shadowed: a local `useContext` is no context read; it returns
// the store `other`.
function Shadowed() {
  const useContext = (_: number) => ({ s: other });
  const { s } = useContext(0);
  createRoot(() => {
    s.v = 18;
  });
  return <div />;
}

const PlainCtx = createContext<{ box: V }>();

// Silent: every provider passes a fresh literal.
function ContextPlain() {
  const ctx = useContext(PlainCtx);
  createRoot(() => {
    ctx.box.v = 19;
  });
  return <div />;
}

// Violation: a tuple slot holding a props member.
function Tuple(props: Props) {
  const pair = [0, props.s] as const;
  const [, it] = pair;
  createRoot(() => {
    it.v = 20;
  });
  return <div />;
}

// Namespace: a store read as a namespace import's member.
function Namespace() {
  const alias = stores.shared;
  createRoot(() => {
    alias.v = 21;
  });
  return <div />;
}

// Uncertain: a tag created inside a call that may be a setter's.
function InWindow(props: { s: V }) {
  props.s.v = 22;
  return <div />;
}

export function App() {
  const [s1, setS1] = createStore({ v: 0 });
  const [s2, setS2] = createStore({ v: 0 });
  const [s3] = createStore({ v: 0, a: { v: 0 } });
  const [s4, setS4] = createStore({ v: 0 });
  setS4(() => {
    void (<InWindow s={s4} />);
  });
  return (
    <div>
      <PropMember s={s1} setS={setS1} />
      <PropSlot s={s1} setS={setS1} />
      <PropMerge s={s1} setS={setS1} />
      <PropOmit s={s1} setS={setS1} extra={1} />
      <Forward s={s1} setS={setS1} />
      <HookSlot />
      <HookMember />
      <Ctx value={{ s: s2, setS: setS2 }}>
        <ContextSlot />
      </Ctx>
      <Imported />
      <PlainProp box={{ v: 0 }} />
      <Draft s={s1} setS={setS1} />
      <Mixed box={s1} />
      <Mixed box={{ v: 0 }} />
      <InSetter s={s1} setS={setS1} />
      <Nested deep={s3} />
      <HandedOn s={s1} setS={setS1} />
      <Replaced t={s1} />
      <Shadowed />
      <PlainCtx value={{ box: { v: 0 } }}>
        <ContextPlain />
      </PlainCtx>
      <Tuple s={s1} setS={setS1} />
      <Namespace />
    </div>
  );
}
