import { createMemo, createSignal } from "solid-js";

const pause = () => new Promise<void>((done) => setTimeout(done, 20));

// ADR 0351: `api.load()` names the function of a closed `const` object literal.
// V: a read after the function's own await, reached through the call.
export function MemberAsyncReturned() {
  const [value] = createSignal(1);
  const [tick] = createSignal(0);
  const api = {
    async load() {
      await pause();
      return value();
    },
  };
  const data = createMemo(() => {
    tick();
    return api.load();
  });
  return <div>{String(data())}</div>;
}

// V: the awaited call's continuation.
export function MemberAwaited() {
  const [value] = createSignal(1);
  const [tick] = createSignal(0);
  const api = {
    async load() {
      await pause();
      return value();
    },
  };
  const data = createMemo(async () => {
    tick();
    return await api.load();
  });
  return <div>{String(data())}</div>;
}

// V: a synchronous method read after the computation's own await.
export function MemberSyncAfterAwait() {
  const [value] = createSignal(1);
  const [tick] = createSignal(0);
  const config = {
    read() {
      return value();
    },
  };
  const data = createMemo(async () => {
    tick();
    await pause();
    return config.read();
  });
  return <div>{String(data())}</div>;
}

// V: a data property holding an arrow function.
export function MemberArrowProperty() {
  const [value] = createSignal(1);
  const [tick] = createSignal(0);
  const api = {
    load: async () => {
      await pause();
      return value();
    },
  };
  const data = createMemo(() => {
    tick();
    return api.load();
  });
  return <div>{String(data())}</div>;
}

// Quiet: a reassigned property is not the literal's function (no exact callee).
export function MemberReassigned() {
  const [value] = createSignal(1);
  const [tick] = createSignal(0);
  const api = {
    async load() {
      await pause();
      return value();
    },
  };
  api.load = async () => 0;
  const data = createMemo(() => {
    tick();
    return api.load();
  });
  return <div>{String(data())}</div>;
}

// Quiet: a spread may replace the property.
export function MemberSpread() {
  const [value] = createSignal(1);
  const [tick] = createSignal(0);
  const base = { load: async () => 0 };
  const api = {
    ...base,
    async load() {
      await pause();
      return value();
    },
  };
  const data = createMemo(() => {
    tick();
    return api.load();
  });
  return <div>{String(data())}</div>;
}

// Quiet: a computed key may name any property.
export function MemberComputedKey() {
  const [value] = createSignal(1);
  const [tick] = createSignal(0);
  const key = "load";
  const api = {
    [key]: async () => {
      await pause();
      return value();
    },
  };
  const data = createMemo(() => {
    tick();
    return api.load();
  });
  return <div>{String(data())}</div>;
}

// Quiet: `this` inside the literal may reach and write the object.
export function MemberThis() {
  const [value] = createSignal(1);
  const [tick] = createSignal(0);
  const api = {
    async load() {
      await pause();
      return value() + (this as unknown as { n: number }).n;
    },
  };
  const data = createMemo(() => {
    tick();
    return api.load();
  });
  return <div>{String(data())}</div>;
}

// Quiet: the object escapes to a function that may change it.
function register(target: unknown) {
  return target;
}
export function MemberEscapes() {
  const [value] = createSignal(1);
  const [tick] = createSignal(0);
  const api = {
    async load() {
      await pause();
      return value();
    },
  };
  register(api);
  const data = createMemo(() => {
    tick();
    return api.load();
  });
  return <div>{String(data())}</div>;
}

// Quiet: an accessor runs its body on access.
export function MemberGetter() {
  const [value] = createSignal(1);
  const [tick] = createSignal(0);
  const api = {
    get load() {
      return async () => {
        await pause();
        return value();
      };
    },
  };
  const data = createMemo(() => {
    tick();
    return api.load();
  });
  return <div>{String(data())}</div>;
}

// Quiet: a `let` binding may be reassigned.
export function MemberLetBinding() {
  const [value] = createSignal(1);
  const [tick] = createSignal(0);
  let api = {
    async load() {
      await pause();
      return value();
    },
  };
  api = api;
  const data = createMemo(() => {
    tick();
    return api.load();
  });
  return <div>{String(data())}</div>;
}

// Quiet: an optional call is not the plain member read this claim follows.
export function MemberOptionalCall() {
  const [value] = createSignal(1);
  const [tick] = createSignal(0);
  const api = {
    async load() {
      await pause();
      return value();
    },
  };
  const data = createMemo(() => {
    tick();
    return api?.load();
  });
  return <div>{String(data())}</div>;
}

// Quiet: an exported binding is reachable from other modules.
const [moduleValue] = createSignal(1);
export const exportedApi = {
  async load() {
    await pause();
    return moduleValue();
  },
};

export function MemberExported() {
  const [tick] = createSignal(0);
  const data = createMemo(() => {
    tick();
    return exportedApi.load();
  });
  return <div>{String(data())}</div>;
}
