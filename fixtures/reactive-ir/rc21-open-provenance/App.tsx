import * as S from "solid-js";
import { createMemo, createSignal, lazy, Loading, NotReadyError } from "solid-js";

declare const load: any;
declare const peek: () => unknown;
declare const value: any;
const named = (): any => load();
const closed = (): number => 1;
function gate() {
  if (Math.random() < 0.5) throw new NotReadyError("x");
}

const anyCompute = createMemo(() => load());
const unknownCompute = createMemo(() => peek());
const namedCompute = createMemo(named);
const [handed] = createSignal(value);
const notReady = createMemo(() => {
  if (Math.random() < 0.5) throw new NotReadyError("x");
  return 1;
});
const throughNamespace = createMemo(() => {
  if (Math.random() < 0.5) throw new S.NotReadyError("x");
  return 1;
});
const throughHelper = createMemo(() => {
  gate();
  return 1;
});
const literalCompute = createMemo(() => 1);
const closedCompute = createMemo(closed);
const [data] = createSignal({ v: 1 });

class Own extends Error {}
const ownError = createMemo(() => {
  if (Math.random() < 0.5) throw new Own("x");
  return 1;
});
const held = {
  read() {
    return literalCompute() + closedAsync();
  },
};
const closedAsync = createMemo(async () => 1);

const Page = lazy(async () => ({ default: () => null }));

export function Open() {
  const a = anyCompute();
  const b = unknownCompute();
  const c = namedCompute();
  const d = handed();
  const e = notReady();
  const f = throughNamespace();
  const g = throughHelper();
  return (
    <i>
      {a}
      {b}
      {c}
      {d}
      {e}
      {f}
      {g}
    </i>
  );
}

export function Proved() {
  const a = literalCompute();
  const b = closedCompute();
  const c = data();
  const d = ownError();
  return (
    <i>
      {a}
      {b}
      {c.v}
      {d}
    </i>
  );
}

export function Rendered() {
  return (
    <i>
      {anyCompute()}
      {held.read()}
    </i>
  );
}

export function Pages() {
  return (
    <div>
      <Page />
      <Loading fallback={0}>
        <Page />
      </Loading>
    </div>
  );
}
