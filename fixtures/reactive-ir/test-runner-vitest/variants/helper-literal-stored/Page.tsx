import { setCount } from "./Card";
import { later } from "./helpers";

// A write in a literal the helper keeps stays an obligation.
export function Page() {
  later(() => setCount(1));
  return <div />;
}
