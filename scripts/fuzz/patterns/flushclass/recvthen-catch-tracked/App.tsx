import { createSignal, createTrackedEffect, flush } from "solid-js";
const [k, setK] = createSignal(0);
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
  createTrackedEffect(() => { k(); s.catch(() => { flush(); }); });
  setTimeout(() => setK(1));
  return <p>x</p>;
}
