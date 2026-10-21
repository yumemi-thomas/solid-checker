import { createEffect, createSignal } from "solid-js";

const [count] = createSignal(0);

// Each form runs its literal's body inside the call, so the body runs under the
// owner of the code around the call. Module scope has none (ADR 0347 amendment).

// `.call` and `.apply` on a function literal.
export const viaCall = (function () {
  createEffect(count, () => {});
}).call(null);
export const viaApply = (function () {
  createEffect(count, () => {});
}).apply(null, []);

// `new` on a plain function expression runs its body.
export const viaNew = new (function (this: unknown) {
  createEffect(count, () => {});
} as any)();

// A comma expression's value is its last operand, the literal.
export const viaComma = (0, () => {
  createEffect(count, () => {});
})();

// A static field initializer runs at class evaluation, under the code around it.
export class Static {
  static x = (() => {
    createEffect(count, () => {});
  })();
}

// An async literal's code after its first `await` is a continuation.
export const viaContinuation = (async function () {
  await Promise.resolve();
  createEffect(count, () => {});
}).call(null);

// Controls. A generator's body never runs when the call only makes its iterator.
export const viaGenerator = (function* () {
  createEffect(count, () => {});
  yield 1;
}).call(null);

// Inside a component the owner is current, so the same forms stay silent. `Comp`
// is a component because a tag renders it.
export function Comp() {
  (function () {
    createEffect(count, () => {});
  }).call(null);
  (0, () => {
    createEffect(count, () => {});
  })();
  return null;
}
export const rendered = <Comp />;
