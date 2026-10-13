import "./Mutate";
import { PrototypeSet } from "reactive-package";
export function MutatedPrototype() {
  const selected = new PrototypeSet<number>();
  return <div>{String(selected.has(1))}</div>;
}
