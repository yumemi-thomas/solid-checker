// Clean. Mirrors dashiboard-ui dashiboard-ui/src/persist.ts:48-50: `getOwner() ? fn() : createRoot(fn)` owns a
// memo and an effect wherever the caller runs, at module scope or in a component.
import { createEffect, createMemo, createRoot, createSignal, getOwner } from "solid-js";

function withOwner<T>(fn: () => T): T {
  return getOwner() ? fn() : createRoot(fn);
}

function persisted(initial: string) {
  const [value, setValue] = createSignal(initial);
  const json = withOwner(() => {
    const memo = createMemo(() => JSON.stringify(value()));
    createEffect(memo, (text) => console.log(text));
    return memo;
  });
  return [value, setValue, json] as const;
}

const [name, setName, nameJson] = persisted("ada");

export function Name() {
  return <button type="button" onClick={() => setName(name() + "!")}>{nameJson()}</button>;
}
