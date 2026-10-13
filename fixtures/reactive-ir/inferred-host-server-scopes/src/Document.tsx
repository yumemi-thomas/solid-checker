import { startClosed } from "reactive-package";
import { createEffect } from "@solidjs/signals";
startClosed();
createEffect(() => 0, () => {});
export default function Document() {
  return <div />;
}
