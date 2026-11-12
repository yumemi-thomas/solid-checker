import { action, flush } from "solid-js";
class Sync<T> extends Promise<T> {
  override then<A = T, B = never>(
    ok?: ((value: T) => A | PromiseLike<A>) | null,
    bad?: ((reason: unknown) => B | PromiseLike<B>) | null,
  ): Promise<A | B> {
    void ok;
    bad?.(undefined);
    return this as unknown as Promise<A | B>;
  }
}
const s = new Sync<void>(() => {});
export function App() {
  const run = action(function* () {
    s.finally(() => { flush(); });
  });
  setTimeout(() => void run());
  return <p>x</p>;
}
