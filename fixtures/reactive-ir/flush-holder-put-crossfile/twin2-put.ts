import { flush } from "solid-js";

export function putTwin2(t: any) {
  t.twin2 = function () {
    flush();
  };
}
