import { makeTuple } from "./factory";
import "./mutations";

export function TupleRead() {
  const [, peek] = makeTuple();
  peek();
  return <main />;
}

export function AccessorRead() {
  const [read] = makeTuple();
  read();
  return <main />;
}
