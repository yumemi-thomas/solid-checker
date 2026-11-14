import { flush } from "solid-js";

export function putN1(t: any) {
  t.n1 = function () {
    flush();
  };
}
