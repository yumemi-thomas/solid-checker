import { createSignal } from "solid-js";
import * as L from "./literal";

const [, setW] = createSignal(0);

export function Namespace() {
  (L.t as unknown as Promise<void>).then(() => { setW(10); });
  return <p />;
}
