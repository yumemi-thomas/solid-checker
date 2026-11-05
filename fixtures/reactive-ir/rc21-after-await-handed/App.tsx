import { createEffect, createMemo, createSignal, createStore, Loading } from "solid-js";

const wait = (ms: number) => new Promise<number>((done) => setTimeout(() => done(ms), ms));
const [s] = createSignal(1);
const [store] = createStore({ v: 0 });

const reads = () => s();
const quiet = () => 1;
const afterParam = async (f: () => number) => {
  await wait(1);
  return f();
};
const beforeParam = async (f: () => number) => {
  const v = f();
  await wait(1);
  return v;
};

export function Handlers() {
  const named = createMemo(() => wait(1).then(reads));
  const silent = createMemo(() => wait(1).then(quiet));
  const param = createMemo(() => afterParam(s));
  const before = createMemo(() => beforeParam(s));
  return (
    <Loading fallback={0}>
      <i>
        {named()}
        {silent()}
        {param()}
        {before()}
      </i>
    </Loading>
  );
}

export function Destructures() {
  createEffect(
    () => 1,
    () => {
      let v = 0;
      ({ v } = store);
      void v;
    },
  );
  return (
    <i
      onClick={() => {
        const [{ v }] = [store];
        void v;
      }}
    />
  );
}
