import { flush } from "solid-js";

export default function putN5(t: any) {
  t.n5 = function () {
    flush();
  };
}
