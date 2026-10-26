// app-game math-examples img-onload-test/createRootMapPool.ts:50: a signal's
// accessor is passed to a factory the pool receives as a parameter, whose body no
// analysis reads.
import { createSignal, type Accessor } from "solid-js";

export function createPool<T>(factory: (key: string, active: Accessor<boolean>) => T) {
  const mapRoot = (key: string): T => {
    const [active] = createSignal(true);
    return factory(key, active);
  };
  return { mapRoot };
}
