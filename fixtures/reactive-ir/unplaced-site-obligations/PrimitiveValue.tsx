import { createEffect, createSignal } from "solid-js";
import * as Solid from "solid-js";

const [count] = createSignal(0);
// Control: a direct call at module scope is a proven unowned effect (SC4001).
createEffect(() => count(), () => {});
// A method of the primitive takes other arguments in other slots (`bind`'s
// one argument is no missing effect function): not a primitive call, so the
// primitive is a value here (SC9012), never SC7001.
createEffect.call(null, () => count(), () => {});
createEffect.bind(null)(() => count(), () => {});

// A primitive used as a value: the calls it reaches are not followed (SC9012).
Reflect.apply(createEffect, null, [() => count(), () => {}]);
const effect = createEffect;
effect(() => count(), () => {});
// Also through a namespace and nested in an expression.
export const viaNamespace = Solid.createEffect;
export const inArray = [createEffect];
declare const flag: boolean;
export const picked = flag ? createEffect : null;

// Type positions and re-exports are not value uses.
export type Effect = typeof createEffect;
export { createEffect };
