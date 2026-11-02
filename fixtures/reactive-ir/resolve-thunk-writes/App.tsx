import { action, createSignal, resolve, until } from "solid-js";

// `resolve(fn)` and `until(fn)` run `fn` as a computation's compute under a
// fresh root: a write or an action call there throws, directly or in a layer
// run in place; a read is tracked by that computation.
export function ThunkWrites() {
  const [r1, setR1] = createSignal(0);
  const act = action(function* () {
    setR1(9);
  });
  void resolve(() => {
    setR1(1);
    return r1();
  });
  void until(() => {
    act();
    return true;
  });
  void resolve(() => {
    [1].forEach(() => setR1(2));
    return 1;
  });
  return <div />;
}

// Past an `await` the async thunk's rest runs on a later task, with no owner:
// legal. Reads in the thunk are legal everywhere.
export function ThunkLegal() {
  const [r1, setR1] = createSignal(0);
  void resolve(async () => {
    await Promise.resolve();
    setR1(3);
    return r1();
  });
  void resolve(() => r1());
  return <button onClick={() => setR1(4)}>go</button>;
}
