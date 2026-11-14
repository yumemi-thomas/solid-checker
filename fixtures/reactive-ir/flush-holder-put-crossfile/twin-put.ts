import { flush } from "solid-js";

export function putTwin(t: any) {
  t.twin = function () {
    flush();
  };
}
