import { useContext } from "solid-js";
import { CrossAccount } from "./ContextStability";

export function replaceCrossAccount() {
  const replaced = useContext(CrossAccount);
  replaced.stale = () => 0;
}
