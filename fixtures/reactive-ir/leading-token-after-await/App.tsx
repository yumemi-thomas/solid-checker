import { createMemo, createSignal } from "solid-js";

async function load(): Promise<number> {
  return 1;
}
const [count] = createSignal(1);

// CLEAN: only `fetchMember`'s expando member is the computation; its leading
// token named `fetchMember`, whose read after the await never runs here.
async function fetchMember() {
  await load();
  return count();
}
fetchMember.sync = () => 0;
export function ExpandoComputation() {
  createMemo(fetchMember.sync);
  return <div />;
}

// VIOLATION: `fetchCalled` is the computation; `count()` follows the await.
async function fetchCalled() {
  await load();
  return count();
}
export function IdentifierComputation() {
  createMemo(fetchCalled);
  return <div />;
}

const makeBox = () => ({ value: 1 });
type Props = { load: () => { value: number }; value: number };

// CLEAN at `.value`: `props.load().value` reads the returned box, not the
// reactive prop `value`; `props.load` itself is static at every call site.
function CallRooted(props: Props) {
  const total = createMemo(async () => {
    await load();
    return props.load().value;
  });
  return <div>{total()}</div>;
}
// VIOLATION: `props.value` is the reactive prop, read after the await.
function PropRooted(props: Props) {
  const total = createMemo(async () => {
    await load();
    return props.value;
  });
  return <div>{total()}</div>;
}

type NestedProps = { inner: { text: string }; text: string };
// VIOLATION: `props.inner.text` reads the reactive prop `inner` after the
// await; the leaf `text` is a static prop (before: read as prop `text`).
function NestedRooted(props: NestedProps) {
  const total = createMemo(async () => {
    await load();
    return props.inner.text;
  });
  return <div>{total()}</div>;
}
// CLEAN: the root's own member `inner` is static, though `text` is reactive.
function NestedStatic(props: NestedProps) {
  const total = createMemo(async () => {
    await load();
    return props.inner.text;
  });
  return <div>{total()}</div>;
}

export function App() {
  return (
    <div>
      <NestedRooted inner={{ text: String(count()) }} text="plain" />
      <NestedStatic inner={{ text: "plain" }} text={String(count())} />
      <CallRooted load={makeBox} value={count()} />
      <PropRooted load={makeBox} value={count()} />
    </div>
  );
}
