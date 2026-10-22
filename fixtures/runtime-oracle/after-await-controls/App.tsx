import { createMemo, createSignal, untrack, Loading } from "solid-js";

const pause = () => new Promise<void>((done) => setTimeout(done, 20));

// Every component here is quiet at runtime and must stay silent in the checker.

export function MapBeforeAwait() {
  const [value] = createSignal(1);
  const data = createMemo(async () => {
    const rows = [1].map(() => value());
    await pause();
    return rows[0];
  });
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}

export function StoredCallback() {
  const [value] = createSignal(1);
  const data = createMemo(async () => {
    await pause();
    const later = () => value();
    return later;
  });
  return <Loading fallback="loading"><span>{typeof data()}</span></Loading>;
}

export function DeferredCallback() {
  const [value] = createSignal(1);
  const data = createMemo(async () => {
    await pause();
    setTimeout(() => void value(), 0);
    return 1;
  });
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}

export function ProjectMap() {
  const [value] = createSignal(1);
  const box = {
    map(callback: () => number) {
      return () => callback();
    },
  };
  const data = createMemo(async () => {
    await pause();
    return box.map(() => value());
  });
  return <Loading fallback="loading"><span>{typeof data()}</span></Loading>;
}

// A thenable that is not a native promise: the runtime never attributes it.
export function ThenOnThenable() {
  const [value] = createSignal(1);
  const thenable: PromiseLike<number> = {
    then(onFulfilled) {
      return Promise.resolve(onFulfilled ? onFulfilled(value()) : 0) as never;
    },
  };
  const data = createMemo(() => thenable.then(() => value()));
  return <Loading fallback="loading"><span>{String(data())}</span></Loading>;
}

export function ThenResultDropped() {
  const [value] = createSignal(1);
  const data = createMemo(() => {
    void pause().then(() => value());
    return 1;
  });
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}

export function HelperInEventHandler() {
  const [value] = createSignal(1);
  async function read() {
    await pause();
    return value();
  }
  return <button onClick={() => void read()}>go</button>;
}

export function HelperResultIgnored() {
  const [value] = createSignal(1);
  async function read() {
    await pause();
    return value();
  }
  const data = createMemo(() => {
    void read();
    return 1;
  });
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}

export function HelperReadsFirst() {
  const [value] = createSignal(1);
  async function read() {
    const seen = value();
    await pause();
    return seen;
  }
  const data = createMemo(() => read());
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}

export function AlreadyTracked() {
  const [value] = createSignal(1);
  const data = createMemo(async () => {
    value();
    await pause();
    return [1].map(() => value())[0];
  });
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}

export function AlreadyTrackedThen() {
  const [value] = createSignal(1);
  const data = createMemo(() => {
    value();
    return pause().then(() => value());
  });
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}

export function UntrackedCallback() {
  const [value] = createSignal(1);
  const data = createMemo(async () => {
    await pause();
    return untrack(() => [1].map(() => value())[0]);
  });
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}

export function UntrackedHelper() {
  const [value] = createSignal(1);
  async function read() {
    await pause();
    return untrack(() => value());
  }
  const data = createMemo(() => read());
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}

export function PlainAsync() {
  const [value] = createSignal(1);
  async function read() {
    await pause();
    return [1].map(() => value())[0];
  }
  void read();
  return <div />;
}

export function SyncHelperBeforeAwait() {
  const [value] = createSignal(1);
  function read() {
    return value();
  }
  const data = createMemo(async () => {
    const seen = read();
    await pause();
    return seen;
  });
  return <Loading fallback="loading"><span>{data()}</span></Loading>;
}

export function SyncHelperReturnsClosure() {
  const [value] = createSignal(1);
  function make() {
    return () => value();
  }
  const data = createMemo(async () => {
    await pause();
    return make();
  });
  return <Loading fallback="loading"><span>{typeof data()}</span></Loading>;
}

export function SyncHelperInHandler() {
  const [value] = createSignal(1);
  function read() {
    return value();
  }
  return <button onClick={() => void read()}>go</button>;
}
