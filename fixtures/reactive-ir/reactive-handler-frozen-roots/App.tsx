import { createSignal } from "solid-js";

const plain = { f: () => {} };
const useHandlers = () => plain;
const box = new Map([["s", plain]]);

export function Roots() {
  const [count] = createSignal(0);
  const fromHook = useHandlers();
  return (
    <>
      <button onClick={fromHook.f}>{count()}</button>
      <button onClick={box.get("s")!.f}>{count()}</button>
      <button onClick={useHandlers().f}>{count()}</button>
      <button onClick={plain.f}>{count()}</button>
    </>
  );
}
