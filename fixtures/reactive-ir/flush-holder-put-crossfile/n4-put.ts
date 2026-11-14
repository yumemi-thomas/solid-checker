import { flush } from "solid-js";

export function putN4(t: any) {
  t.n4 = function () {
    flush();
  };
}
