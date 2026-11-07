import { createMemo, createSignal } from "solid-js";

const any: any = () => 1;
declare const options: { sync: boolean };
const named = () => 1;
// ADR 0865: an async generator's iterator has no `then`, and `handleAsync` probes
// `Symbol.asyncIterator` too. A sync generator's iterator is a plain value.
async function* asyncGenerator() {
  yield 1;
}
const heldGenerator = async function* () {
  yield 1;
};
function* syncGenerator() {
  yield 1;
}
const holder = {
  async *generate() {
    yield 1;
  },
};

export function Computes(props: { compute: () => unknown; flag: boolean }) {
  const [count] = createSignal(0);
  createMemo(async () => count(), { sync: true });
  createMemo(() => Promise.resolve(count()), { sync: true });
  createMemo(() => any(), { sync: true });
  createMemo(props.compute, { sync: true });
  createMemo(() => (props.flag ? Promise.resolve(1) : 1), { sync: true });
  createMemo(() => any(), options);
  createMemo(() => count(), { sync: true });
  createMemo(named, { sync: true });
  createMemo(() => count(), options);
  createMemo(asyncGenerator, { sync: true });
  createMemo(heldGenerator, { sync: true });
  createMemo(async function* () {
    yield 1;
  }, { sync: true });
  createMemo(asyncGenerator, options);
  createMemo(syncGenerator, { sync: true });
  createMemo(() => holder.generate(), { sync: true });
  return <i />;
}
