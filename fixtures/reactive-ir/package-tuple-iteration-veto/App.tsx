import { createTicker } from "reactive-package";
import "./mutations";

export function TupleContractRead() {
  const [, start] = createTicker();
  start();
  return <div />;
}
