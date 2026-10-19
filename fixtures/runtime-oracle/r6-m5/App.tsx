import "./runtime";
declare const opaque: { run: () => void };
declare function makeOpaque(): { run: () => void };
declare const partial: { run?: () => void; other?: number };
declare function opaqueFn(o: { run: () => void }): void;
import { api } from "./barrel";
import { patch } from "./mutate";
type Runner = { run: () => void };
function helper(o: Runner) {
  o.run();
}
export function ViaHelper() {
  patch(opaque as unknown as Record<string, unknown>);
  helper(api);
  return <main />;
}
export function Direct() {
  patch(opaque as unknown as Record<string, unknown>);
  api.run();
  return <main>{api.read()}</main>;
}
export function DirectRead() {
  patch({});
  const v = api.read();
  return <main>{v}</main>;
}
