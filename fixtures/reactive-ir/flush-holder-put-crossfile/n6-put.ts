import { flush } from "solid-js";

export function putN6(t: any) {
  t.n6 = function () {
    flush();
  };
}
