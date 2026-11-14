import { flush } from "solid-js";

export function putN3(t: any) {
  t.n3 = function () {
    flush();
  };
}
