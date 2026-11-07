import { bindEvent } from "./events";
function notify(run: () => void) {
  return () => run();
}
bindEvent(window, "x", notify(() => {}));
export {};
