import { Loading, createMemo, createSignal } from "solid-js";
import useVals from "./hooks";

function Child() {
  return <p>child</p>;
}

// Uncertifiable (SC5006): the boundary re-arms through `on`, and the async
// source it waits on is also read outside it (rc.17 in Chrome:
// LOADING_ON_OUTSIDE_HOLD after the click). ADR 0517.
export function OutsideRead() {
  const [t, setT] = createSignal(1);
  const a = createMemo(async () => { t(); await Promise.resolve(); return 1; });
  return (
    <div>
      <Loading fallback={0} on={t()}>
        <p>{a()}</p>
      </Loading>
      <p>{a()}</p>
      <button onClick={() => setT(2)}>go</button>
    </div>
  );
}

// Uncertifiable: a component under the boundary renders reads not shown here.
export function ComponentChild() {
  const [t, setT] = createSignal(1);
  return (
    <div>
      <Loading fallback={0} on={t()}>
        <Child />
      </Loading>
      <button onClick={() => setT(2)}>go</button>
    </div>
  );
}

// Clean: only the boundary reads the source.
export function InsideOnly() {
  const [t, setT] = createSignal(1);
  const a = createMemo(async () => { t(); await Promise.resolve(); return 1; });
  return (
    <div>
      <Loading fallback={0} on={t()}>
        <p>{a()}</p>
      </Loading>
      <button onClick={() => setT(2)}>go</button>
    </div>
  );
}

// Clean for this rule: without `on` the boundary never re-arms.
export function NoOn() {
  const [t, setT] = createSignal(1);
  const a = createMemo(async () => { t(); await Promise.resolve(); return 1; });
  return (
    <div>
      <Loading fallback={0}>
        <p>{a()}</p>
      </Loading>
      <p>{a()}</p>
      <button onClick={() => setT(2)}>go</button>
    </div>
  );
}

// Uncertifiable: the children come through an attribute.
export function ChildrenAttribute() {
  const [t, setT] = createSignal(1);
  const a = createMemo(async () => { t(); await Promise.resolve(); return 1; });
  return (
    <div>
      <Loading fallback={0} on={t()} children={<p>{a()}</p>} />
      <p>{a()}</p>
      <button onClick={() => setT(2)}>go</button>
    </div>
  );
}

// Uncertifiable: the async memo comes from a hook in another module, so the
// read under the boundary resolves to no source the analysis sees.
export function HookSource() {
  const { t, setT, a } = useVals();
  return (
    <div>
      <Loading fallback={0} on={t()}>
        <p>{a()}</p>
      </Loading>
      <p>{a()}</p>
      <button onClick={() => setT(2)}>go</button>
    </div>
  );
}
