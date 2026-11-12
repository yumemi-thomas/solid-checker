import { createMemo, createOptimistic, createSignal } from "solid-js";

// Batch 11, slice 4: a setter call carries Chrome's OPTIMISTIC_WRITE_BEFORE_LOAD (SC2009) beside REACTIVE_WRITE_IN_OWNED_SCOPE (SC2001) when its source may be async, or when no
// fact says which primitive made it. The code is ended by SC2009's own row, a silence that names it, or the code's uncertifiable row `optimistic-write-unaccounted`.

// An async optimistic source, the setter called before its first answer (rc.21 throws OPTIMISTIC_WRITE_BEFORE_LOAD, ahead of any owned-scope guard): SC2009's violation stands.
// SC2001's code has no row of its own there: `write-unaccounted` stands beside it.
export function Async() {
  const [, setValue] = createOptimistic(async () => 1);
  setValue(2);
  return <p>x</p>;
}

// The same through an alias of the setter (the alias row's path): the same rows at the write.
export function Aliased() {
  const [, setValue] = createOptimistic(async () => 1);
  const write = setValue;
  write(2);
  return <p>x</p>;
}

// A sync source has its answer: no OPTIMISTIC code, no SC2009 row and no `optimistic-write-unaccounted`.
export function Sync() {
  const [, setValue] = createOptimistic(1);
  setValue(2);
  return <p>x</p>;
}

// A setter reached through an object slot of a `const` arrow's result (ADR 0480's alias): the alias is the `createSignal` setter for SC2001, but no fact names its primitive
// for SC2009, so the OPTIMISTIC code is carried (fail closed) and its row, `optimistic-write-unaccounted`, stands beside SC2001's violation. Chrome raises only
// REACTIVE_WRITE_IN_OWNED_SCOPE for a signal's setter: the row is the cost of not knowing.
export function SlotAlias() {
  const [count, setCount] = createSignal(0);
  const useThing = () => ({ useIt: setCount });
  const { useIt } = useThing();
  const doubled = createMemo(() => {
    useIt(7);
    return count() * 2;
  });
  return <p>{doubled()}</p>;
}
