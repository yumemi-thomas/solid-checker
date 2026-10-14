import { createEffect, createRenderEffect, createSignal, onCleanup, untrack } from "solid-js";
import * as Solid from "solid-js";
import type { EffectOptions } from "solid-js";
import { emptyDependency, readDependency } from "./dependencies";

export function App(props: { compute: () => number; options: EffectOptions; defer: boolean }) {
  const [count, setCount] = createSignal(0);
  // Clean: no compute source can schedule these skipped initial applies.
  createEffect(() => undefined, () => { console.log(count()); }, { defer: true });
  createEffect(() => 42, () => { onCleanup(() => {}); }, { defer: true });
  createEffect(() => untrack(count), () => { console.log(count()); }, { defer: true });
  Solid.createRenderEffect(() => undefined, () => { console.log(count()); onCleanup(() => {}); }, { defer: true });
  createEffect(() => 0, {
    effect: () => { console.log(count()); },
    error: () => { console.log(count()); },
  }, { defer: true });

  // Violations: the retained writable source is read by compute.
  createEffect(() => count(), () => { console.log(count()); }, { defer: true });
  Solid.createRenderEffect(() => (count() as number), () => { console.log(count()); onCleanup(() => {}); }, { defer: true });

  // Uncertifiable: compute dependency or the defer value is unknown.
  createEffect(() => props.compute(), () => { console.log(count()); onCleanup(() => {}); }, { defer: true });
  createEffect(() => undefined, () => { console.log(count()); onCleanup(() => {}); }, props.options);
  createRenderEffect(() => count(), () => { console.log(count()); }, { defer: props.defer });
  createEffect(() => undefined, () => { console.log(count()); }, { ...props.options });
  createEffect(() => undefined, () => { console.log(count()); }, { ["defer"]: true });

  // Unchanged: omitted options, an absent key and literal false run initially.
  createEffect(() => undefined, () => { console.log(count()); });
  createEffect(() => undefined, () => { console.log(count()); }, {});
  createEffect(() => undefined, () => { console.log(count()); }, { defer: false });
  createRenderEffect(() => undefined, () => { console.log(count()); }, { defer: false });

  // Named compute/apply and transparent wrappers retain exact identities.
  function constant() { return undefined; }
  function deadApply() { console.log(count()); onCleanup(() => {}); }
  createEffect((constant), deadApply, { defer: true });
  function liveApply() { console.log(count()); }
  createEffect(() => count(), liveApply, { defer: true });
  function mixedApply() { console.log(count()); }
  createEffect(constant, mixedApply, { defer: true });
  createEffect(constant, mixedApply);

  // An unknown callback target must not become a dependency-free proof.
  createEffect(props.compute, () => { console.log(count()); }, { defer: true });
  // A conditional dependency is not a guaranteed initial subscription.
  createEffect(() => props.defer ? count() : 0, () => { console.log(count()); }, { defer: true });
  // A retained setter alone cannot defeat a comparator that blocks changes.
  const [frozen, setFrozen] = createSignal(0, { equals: () => true });
  createEffect(() => frozen(), () => { console.log(count()); }, { defer: true });
  void setFrozen;
  createEffect(emptyDependency, () => { console.log(count()); onCleanup(() => {}); }, { defer: true });
  createEffect(readDependency, () => { console.log(count()); }, { defer: true });
  function shadowed() {
    function count() { return 0; }
    createEffect(count, () => { console.log(count()); }, { defer: true });
  }
  shadowed();
  // The same premise gates primitives created by a ref inside apply.
  createEffect(() => 0, () => {
    const element = <button ref={() => { createEffect(() => 0, () => {}); }} />;
    void element;
  }, { defer: true });
  createEffect(() => 0, () => {
    const element = <button ref={() => { createEffect(() => 0, () => {}); }} />;
    void element;
  }, props.options);
  createEffect(() => 0, () => {
    const element = <button ref={() => { createEffect(() => 0, () => {}); }} />;
    void element;
  }, { defer: false });
  return <button onClick={() => setCount(value => value + 1)}>{count()}</button>;
}
