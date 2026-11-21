import { createEffect, createStore } from "solid-js";
import { retypeAll } from "dep-retype";

export function Misuse() {
  const [s] = createStore({ v: 0 });
  return <input type="checkbox" onChange={() => { createEffect(() => s.v, () => {}); }} />;
}

// A dependency needs no element handed to it: `retypeAll` (its source is analysed, `dependencySources`) queries every input in the document and writes its `type`.
export const retype = retypeAll;
