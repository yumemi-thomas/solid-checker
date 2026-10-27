// compass-ui Bridge.tsx:375-386: a createEffect with `{ defer: true }` whose apply callback reads a signal.
import { createEffect, createSignal } from "solid-js";

export function Board(props: { stops: string[] }) {
  const [tab] = createSignal("a");
  let prevTab = "";
  createEffect(
    () => props.stops,
    (list) => {
      const current = tab();
      if (current !== prevTab) console.log(list.length);
      prevTab = current;
    },
    { defer: true },
  );
  return <p>{props.stops.length}</p>;
}
