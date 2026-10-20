import { createSignal } from "solid-js";

// Written after an `await` the continuation has no owner, so this write is
// legal wherever `check()` was started.
export function createDataStream() {
  const [ok, setOk] = createSignal(false);
  const check = async () => {
    try {
      const response = await fetch("/health");
      setOk(response.ok);
    } catch {
      setOk(false);
    }
  };
  check();
  return { ok };
}

// The first statement runs synchronously in the caller: a write there is a
// write in the caller's owned scope.
export function connect() {
  const [live, setLive] = createSignal(true);
  async function open() {
    setLive(false);
    await fetch("/open");
  }
  open();
  return live;
}
