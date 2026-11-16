import { Errored, For, Repeat, Show, createContext, useContext } from "solid-js";

// A default: a read with no owner throws NoOwnerError, a read under one returns 7.
const Ctx = createContext(7);
// Default-less and never provided: a read under an owner throws ContextNotFoundError.
const Bare = createContext<number>();

// Violations: the render runs the function in place, and nothing above takes its throw.
export function RefArray() {
  return <p ref={[() => { useContext(Ctx); }, () => {}]}>x</p>;
}

const on: boolean = true;

export function RefBranch() {
  return <p ref={on ? () => { useContext(Ctx); } : undefined}>x</p>;
}

export function RefFactory() {
  const directive = () => {
    return () => { useContext(Ctx); };
  };
  return <p ref={directive()}>x</p>;
}

export function RefInRow() {
  return (
    <div>
      <For each={[1]}>{() => <li><p ref={() => { useContext(Ctx); }}>x</p></li>}</For>
    </div>
  );
}

export function RefInRepeat() {
  return (
    <div>
      <Repeat count={1}>{() => <li><p ref={() => { useContext(Ctx); }}>x</p></li>}</Repeat>
    </div>
  );
}

export function RefInShowChild() {
  return (
    <div>
      <Show when={1}>{(value) => <p ref={() => { useContext(Ctx); }}>{value()}</p>}</Show>
    </div>
  );
}

export function RowBody() {
  return (
    <div>
      <For each={[1]}>{() => { useContext(Bare); return <li>x</li>; }}</For>
    </div>
  );
}

export function RowAsync() {
  return (
    <div>
      <For each={[1]}>{async () => { await Promise.resolve(); useContext(Ctx); return <li>x</li>; }}</For>
    </div>
  );
}

// Obligations: a boundary above takes the throw, no exact fact says the render runs the function, or the control-flow tag is not
// written in place in the JSX its function returns (its compute stores the row's throw, and nothing is proven to read it).
export function RefArrayHandled() {
  return (
    <Errored fallback={(error) => <i>{String(error())}</i>}>
      <p ref={[() => { useContext(Ctx); }, () => {}]}>x</p>
    </Errored>
  );
}

export function RefInRowHandled() {
  return (
    <Errored fallback={(error) => <i>{String(error())}</i>}>
      <For each={[1]}>{() => <li><p ref={() => { useContext(Ctx); }}>x</p></li>}</For>
    </Errored>
  );
}

export function RowBodyHandled() {
  return (
    <Errored fallback={(error) => <i>{String(error())}</i>}>
      <For each={[1]}>{() => { useContext(Bare); return <li>x</li>; }}</For>
    </Errored>
  );
}

const maker = {
  make() {
    return () => { useContext(Ctx); };
  },
};

export function RefArrayAsyncCaught() {
  return <p ref={[() => { (async () => { await Promise.resolve(); useContext(Ctx); })().catch(() => {}); }]}>x</p>;
}

export function RowAsyncCaught() {
  return (
    <div>
      <For each={[1]}>{() => { (async () => { await Promise.resolve(); useContext(Ctx); })().catch(() => {}); return <li>x</li>; }}</For>
    </div>
  );
}

export function UnusedFor() {
  const unused = <For each={[1]}>{() => <li><p ref={() => { useContext(Ctx); }}>x</p></li>}</For>;
  void unused;
  return <div>x</div>;
}

export function UnusedRowBody() {
  const unused = <For each={[1]}>{() => { useContext(Bare); return <li>x</li>; }}</For>;
  void unused;
  return <div>x</div>;
}

export function UnusedShow() {
  const unused = <Show when={1}>{(value) => <p ref={() => { useContext(Ctx); }}>{value()}</p>}</Show>;
  void unused;
  return <div>x</div>;
}

function Drop(props: { children?: unknown }) {
  void props;
  return <div></div>;
}

export function DroppedFor() {
  return <Drop><For each={[1]}>{() => <li><p ref={() => { useContext(Ctx); }}>x</p></li>}</For></Drop>;
}

export function RowInExpression(props: { on: boolean }) {
  return <div>{props.on && <For each={[1]}>{() => <li><p ref={() => { useContext(Ctx); }}>x</p></li>}</For>}</div>;
}

export function RefMemberFactory() {
  return <p ref={maker.make()}>x</p>;
}

export function RefRebound() {
  let apply = () => { useContext(Ctx); };
  apply = () => {};
  return <p ref={apply}>x</p>;
}

function Mine(props: { children: () => unknown }) {
  void props;
  return <div></div>;
}

export function ProjectChild() {
  return <Mine>{() => <p ref={() => { useContext(Ctx); }}>x</p>}</Mine>;
}

// Silent: a function the ref applies that reads nothing, and a row that reads a context with a default under its own owner.
export function RefArrayQuiet() {
  return <p ref={[() => {}, () => {}]}>x</p>;
}

export function RowReadsDefault() {
  return (
    <div>
      <For each={[1]}>{() => { useContext(Ctx); return <li>x</li>; }}</For>
    </div>
  );
}
