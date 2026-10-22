import { createMemo, createSignal, Loading } from "solid-js";

const pause = () => new Promise<void>((done) => setTimeout(done, 20));

export function AsyncHelperReturned() {
  const [value] = createSignal(1);
  async function read() {
    await pause();
    return value();
  }
  const data = createMemo(() => read());
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}

export function AsyncHelperAwaited() {
  const [value] = createSignal(1);
  async function read() {
    await pause();
    return value();
  }
  const data = createMemo(async () => await read());
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}

export function AsyncHelperCallback() {
  const [value] = createSignal(1);
  async function read() {
    await pause();
    return [1].map(() => value())[0];
  }
  const data = createMemo(async () => {
    const result = await read();
    return result;
  });
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}

export function AsyncHelperChain() {
  const [value] = createSignal(1);
  async function inner() {
    await pause();
    return value();
  }
  async function outer() {
    return (await inner()) + 1;
  }
  const data = createMemo(() => outer());
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}

export function PromiseAllOfHelpers() {
  const [value] = createSignal(1);
  async function read() {
    await pause();
    return value();
  }
  const data = createMemo(async () => (await Promise.all([read(), read()])).length);
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}

export function PromiseAllOfAsyncCallbacks() {
  const [value] = createSignal(1);
  const data = createMemo(async () => {
    return await Promise.all(
      [1].map(async () => {
        await pause();
        return value();
      }),
    );
  });
  return <Loading fallback="loading"><span>{data().join()}</span></Loading>;
}

export function SyncHelper() {
  const [value] = createSignal(1);
  function read() {
    return value();
  }
  const data = createMemo(async () => {
    await pause();
    return read();
  });
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}

export function SyncHelperInCallback() {
  const [value] = createSignal(1);
  const read = () => value();
  const data = createMemo(async () => {
    await pause();
    return [1].map(() => read())[0];
  });
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}

export function SyncHelperInThen() {
  const [value] = createSignal(1);
  function read() {
    return value();
  }
  const data = createMemo(() => pause().then(() => read()));
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}

// The read happens on this call (flag is true), which the summary alone does
// not prove: an obligation, covered as uncertifiable.
export function SyncHelperConditional() {
  const [value] = createSignal(1);
  function read(flag: boolean) {
    return flag ? value() : 0;
  }
  const data = createMemo(async () => {
    await pause();
    return read(true);
  });
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}

export function AsyncReturnsHelper() {
  const [value] = createSignal(1);
  async function read() {
    await pause();
    return value();
  }
  const data = createMemo(async () => read());
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}

export function AsyncHelperReturnsHelper() {
  const [value] = createSignal(1);
  async function inner() {
    await pause();
    return value();
  }
  async function outer() {
    return inner();
  }
  const data = createMemo(() => outer());
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}

export function SyncHelperThroughHelper() {
  const [value] = createSignal(1);
  function inner() {
    return value();
  }
  function outer() {
    return inner() + 1;
  }
  const data = createMemo(async () => {
    await pause();
    return outer();
  });
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}
