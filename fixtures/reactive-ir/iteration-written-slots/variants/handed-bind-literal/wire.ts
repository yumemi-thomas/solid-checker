import { bindEvent } from "./events";
bindEvent(window, "x", () => {});
bindEvent(window, "y", (e) => {
  if (e.persisted) e.preventDefault();
});
export {};
