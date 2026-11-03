import { For, Match, Show, Switch, createContext, createMemo, createRoot, createSignal, onSettled, useContext } from "solid-js";
import type { Accessor } from "solid-js";
import * as S from "solid-js";
import { Panel, Show as LookalikeShow } from "./panel";

// ADR 0570: a pending read is proven only in the synchronous pass that created
// its source. A parent's memo handed by prop or context, or a module-scope
// memo, has settled when a button mounts the reader later, and rc.17 then
// warns only the strict read. The mounts are main.tsx's module-scope renders.

const modMemo = createMemo(async () => 1);
const FirstCtx = createContext<{ a1: Accessor<number> }>();
const LaterCtx = createContext<{ a1: Accessor<number> }>();
const WrapCtx = createContext<{ a1: Accessor<number> }>();

function h1(cb: () => unknown) {
  cb();
  return 1;
}
function h2(cb: () => unknown) {
  cb();
  return 1;
}

function useVals() {
  const a1 = createMemo(async () => 1);
  return { a1 };
}

function FirstPropReader(props: { a1: Accessor<number> }) {
  props.a1();
  return <div />;
}
function FirstCtxReader() {
  const ctx = useContext(FirstCtx);
  createRoot(() => {
    h1(() => {
      ctx.a1();
    });
  });
  return <div />;
}
function FirstModReader() {
  modMemo();
  return <div />;
}
function ShowTrueReader(props: { a1: Accessor<number> }) {
  createRoot(() => {
    props.a1();
  });
  return <div />;
}
function FirstSettledReader(props: { a1: Accessor<number> }) {
  onSettled(() => {
    props.a1();
  });
  return <div />;
}

// Violation (SC5001) at each reader: the parent creates the memo and mounts
// the reader on its own stack (an intrinsic child, a provider, `<Show
// when={true}>`), and main.tsx renders the parent at module scope.
export function FirstParent() {
  const a1 = createMemo(async () => 1);
  return (
    <div>
      <FirstPropReader a1={a1} />
      <FirstCtx value={{ a1 }}>
        <FirstCtxReader />
      </FirstCtx>
      <FirstModReader />
      <Show when={true}>
        <ShowTrueReader a1={a1} />
      </Show>
      <FirstSettledReader a1={a1} />
      <S.Show when={true}>
        <NamespaceTrueReader a1={a1} />
      </S.Show>
      <LookalikeShow when={true}>
        <LookalikeReader a1={a1} />
      </LookalikeShow>
    </div>
  );
}

function LaterPropReader(props: { a1: Accessor<number> }) {
  props.a1();
  return <div />;
}
function LaterCtxReader() {
  const ctx = useContext(LaterCtx);
  ctx.a1();
  return <div />;
}
function LaterModReader() {
  modMemo();
  return <div />;
}
function LaterSettledReader(props: { a1: Accessor<number> }) {
  onSettled(() => {
    props.a1();
  });
  return <div />;
}
function LaterWrapReader() {
  const ctx = useContext(WrapCtx);
  createRoot(() => {
    h2(() => {
      ctx.a1();
    });
  });
  return <div />;
}
function ForReader(props: { a1: Accessor<number> }) {
  props.a1();
  return <div />;
}
function MatchReader(props: { a1: Accessor<number> }) {
  props.a1();
  return <div />;
}
function NamespaceReader(props: { a1: Accessor<number> }) {
  props.a1();
  return <div />;
}
function PanelReader(props: { a1: Accessor<number> }) {
  props.a1();
  return <div />;
}
function NamespaceTrueReader(props: { a1: Accessor<number> }) {
  props.a1();
  return <div />;
}
function LookalikeReader(props: { a1: Accessor<number> }) {
  props.a1();
  return <div />;
}
function JsxReader(props: { a1: Accessor<number> }) {
  return <div>{props.a1()}</div>;
}
// Violation (SC5001): the reader's own hook creates the memo in its body,
// so even a later mount meets it pending.
function HookReader() {
  const vals = useVals();
  vals.a1();
  return <div />;
}

// Uncertifiable, never a violation (each a violation on 719b0e49a): the
// reader mounts when the button sets `on`, `xs` changes or a project
// component renders its children, after the memo settled.
export function LaterParent() {
  const a1 = createMemo(async () => 1);
  const [on, setOn] = createSignal(false);
  const [xs] = createSignal([1]);
  return (
    <div>
      <button onClick={() => setOn(true)}>mount</button>
      <Show when={on()}>
        <LaterPropReader a1={a1} />
      </Show>
      <Show when={on()}>
        <LaterCtx value={{ a1 }}>
          <LaterCtxReader />
        </LaterCtx>
      </Show>
      <Show when={on()}>
        <LaterModReader />
      </Show>
      <Show when={on()}>
        <LaterSettledReader a1={a1} />
      </Show>
      <Show when={on()}>
        <WrapCtx value={{ a1 }}>
          <LaterWrapReader />
        </WrapCtx>
      </Show>
      <For each={xs()}>{() => <ForReader a1={a1} />}</For>
      <Switch>
        <Match when={on()}>
          <MatchReader a1={a1} />
        </Match>
      </Switch>
      <S.Show when={on()}>
        <NamespaceReader a1={a1} />
      </S.Show>
      <Panel>
        <PanelReader a1={a1} />
      </Panel>
      <Show when={on()}>
        <JsxReader a1={a1} />
      </Show>
      <Show when={on()}>
        <HookReader />
      </Show>
    </div>
  );
}
