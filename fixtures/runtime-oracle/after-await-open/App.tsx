import { createMemo, createSignal, Loading } from "solid-js";

const pause = () => new Promise<void>((done) => setTimeout(done, 20));

// Shapes the runtime reports and SC1002 does not prove yet; each is an `open`
// site in oracle.json.

export function PromiseInVariable() {
  const [value] = createSignal(1);
  const data = createMemo(() => {
    const settled = pause().then(() => value());
    return settled;
  });
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}

export function MemberHelper() {
  const [value] = createSignal(1);
  const api = {
    async read() {
      await pause();
      return value();
    },
  };
  const data = createMemo(() => api.read());
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}

export function FetchThen() {
  const [value] = createSignal(1);
  const data = createMemo(() => fetch("data:,").then(() => value()));
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}

export function TypedPromiseParameter(props: { source?: Promise<void> }) {
  const [value] = createSignal(1);
  const source = props.source ?? pause();
  const data = createMemo(() => source.then(() => value()));
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}

export function RaceOfHelpers() {
  const [value] = createSignal(1);
  async function read() {
    await pause();
    return value();
  }
  const data = createMemo(async () => await Promise.race([read()]));
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}

export function AsyncIife() {
  const [value] = createSignal(1);
  const data = createMemo(async () => {
    return await (async () => {
      await pause();
      return value();
    })();
  });
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}

export function ForAwaitThenRead() {
  const [value] = createSignal(1);
  const data = createMemo(async () => {
    for await (const item of [pause()]) void item;
    return value();
  });
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}
