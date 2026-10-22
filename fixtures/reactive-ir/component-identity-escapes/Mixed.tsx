import { createSignal, Show } from "solid-js";
import { Remote } from "./Remote";
import * as Parts from "./Remote";

const [count] = createSignal(0);
const [items] = createSignal(["a", "b"]);

// Uncertifiable: a tag the program never reaches renders it, and `map` calls
// it plainly in tracked JSX, where no strict-read window opens. One tag does
// not account for the other reference.
function Mixed(item?: unknown) {
  const prefix = count();
  return <li>{prefix}:{item}</li>;
}

// Uncertifiable too (the recall cost): the tag is reachable, but `map` still
// runs the body as a plain call, so the one tag proves nothing about the
// other entry.
function MixedReachable(item?: unknown) {
  const prefix = count();
  return <li>{prefix}:{item}</li>;
}

// Uncertifiable: a plain call is a reference the tag does not account for.
function Called() {
  const value = count();
  return <b>{value}</b>;
}

// Positive: a tag renders it, and the module surface only exports it.
function Exported() {
  const value = count();
  return <b>{value}</b>;
}
export { Exported };

export function Page() {
  return (
    <ul>
      <Show when={false}>
        <Mixed />
      </Show>
      {items().map(Mixed)}
      <MixedReachable />
      {items().map(MixedReachable)}
      <Called />
      {Called()}
      <Exported />
      <Remote />
      <Parts.Chip />
    </ul>
  );
}
