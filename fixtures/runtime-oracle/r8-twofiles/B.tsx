import * as Widget from "./widget";
declare function sink(x: unknown): void;
export function leak() {
  sink(Widget);
}
