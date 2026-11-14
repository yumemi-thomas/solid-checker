import { flush } from "solid-js";

export function putN8(t: any) {
  t.n8 = function () {
    flush();
  };
}
