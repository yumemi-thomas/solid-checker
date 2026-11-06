import { createMemo, createSignal, Loading, For, Show } from "solid-js";
import { Dynamic } from "@solidjs/web";

function Kid(props: { run: () => void }) { return <p>{(props.run(), "x")}</p>; }
export function App() {
  const a1 = createMemo(async () => 1);
  const wrapUse = () => { a1(); };
  return <Loading fallback="x"><Kid run={() => { wrapUse(); }} /></Loading>;
}
