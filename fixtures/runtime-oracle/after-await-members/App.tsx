import { Loading, createMemo, createSignal } from "solid-js";

const pause = () => new Promise<void>((done) => setTimeout(done, 20));

// Batch 11, slice 9B (ADR 0973), members: a call of a project method written after the await of a tracked async computation runs that method on the continuation, with no
// observer: a read in it is untracked and Chrome rc.21 reports UNTRACKED_READ_AFTER_AWAIT at the read. The universe lists a call of a project method as a site (exact or not,
// with its row), so a clean here is a false clean. Each export reads its own signal.

// A method of a `const` object literal (exact, ADR 0279).
const [viaObject] = createSignal(1);
const api = {
  read() {
    return viaObject();
  },
};
export function ByObjectMethod() {
  const data = createMemo(async () => {
    await pause();
    return api.read();
  });
  return (
    <Loading fallback="loading">
      <span>{data()}</span>
    </Loading>
  );
}

// A method of a `const` instance of a class nothing subclasses (exact, ADR 0279).
const [viaClass] = createSignal(2);
class Store {
  read() {
    return viaClass();
  }
}
const store = new Store();
export function ByClassInstance() {
  const data = createMemo(async () => {
    await pause();
    return store.read();
  });
  return (
    <Loading fallback="loading">
      <span>{data()}</span>
    </Loading>
  );
}

// `this.helper()` in a class's own async method, which is the computation: the compute is an arrow that returns the promise the method makes.
const [viaThis] = createSignal(3);
class Loader {
  async load() {
    await pause();
    return this.helper();
  }
  helper() {
    return viaThis();
  }
}
const loader = new Loader();
export function ByThisHelper() {
  const data = createMemo(() => loader.load());
  return (
    <Loading fallback="loading">
      <span>{data()}</span>
    </Loading>
  );
}

// A receiver that is a parameter typed as the class: the helper the continuation calls reads through its parameter.
const [viaParameter] = createSignal(4);
class Source {
  read() {
    return viaParameter();
  }
}
function readFrom(source: Source) {
  return source.read();
}
const source = new Source();
export function ByParameterReceiver() {
  const data = createMemo(async () => {
    await pause();
    return readFrom(source);
  });
  return (
    <Loading fallback="loading">
      <span>{data()}</span>
    </Loading>
  );
}
