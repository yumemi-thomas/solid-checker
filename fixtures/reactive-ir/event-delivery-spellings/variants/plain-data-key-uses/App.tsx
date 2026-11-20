import { createEffect, createSignal } from "solid-js";

const [count] = createSignal(0);
const rows = [{ done: false, run: (_event: Event) => {} }];
declare function key(): string;

// ADR 1133 round 1 (F6): the object of a member under a plain data key is inert whatever the member's own use: its key is no standard
// function-valued name, route, hook, private key or delivery name, so what it holds is something the program put there. A first cut
// held a direct call and let the same call through a name or `.call` pass; no unsafe program was found, so the test is gone and
// the rule is the key's. The read stays proved (the unresolved calls keep the SC9012 rows they have).
export function Page() {
  const box = (<div onScroll={() => { count(); }} />) as HTMLDivElement;
  createEffect(
    () => 1,
    () => {
      box.dispatchEvent(new Event("scroll"));
      (rows as any)[key()].done = true;
      (rows as any)[key()].run(new Event("click"));
      const run = (rows as any)[key()].run;
      run(new Event("click"));
      (rows as any)[key()].run.call(null, new Event("click"));
    },
  );
  return <div />;
}
