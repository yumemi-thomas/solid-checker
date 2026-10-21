import { createEffect, createReaction, createSignal, onCleanup } from "solid-js";

const [count] = createSignal(0);

// TypeScript wrappers keep a callback's value: `h as X`, `h!` and
// `h satisfies X` are the function h, so each form names h's identity.
// Every callback below creates an effect or registers a cleanup with no owner.

// createReaction: as, non-null, satisfies, and a wrapper call under `as`.
function reactAs() {
  onCleanup(() => {});
}
createReaction(reactAs as () => void)(() => {});

function reactNonNull() {
  onCleanup(() => {});
}
createReaction(reactNonNull!)(() => {});

function reactSatisfies() {
  onCleanup(() => {});
}
createReaction(reactSatisfies satisfies () => void)(() => {});

function wrapReaction(callback: () => void): () => void {
  return callback;
}
createReaction(wrapReaction(() => onCleanup(() => {})) as any)(() => {});

// onClick: as, non-null, satisfies.
function clickAs() {
  createEffect(count, () => {});
}
function clickNonNull() {
  createEffect(count, () => {});
}
function clickSatisfies() {
  createEffect(count, () => {});
}

// ref: as and non-null.
function refAs(_el: unknown) {
  createEffect(count, () => {});
}
function refNonNull(_el: unknown) {
  createEffect(count, () => {});
}

export function Controls() {
  return (
    <div>
      <button onClick={clickAs as any} />
      <button onClick={clickNonNull!} />
      <button onClick={clickSatisfies satisfies () => void} />
      <div ref={refAs as any} />
      <div ref={refNonNull!} />
    </div>
  );
}

// Negative: a wrapper around a call names the call's value, which is not the
// callee (the factory's returned handler is unknown), so the sugar stays silent.
function makeHandler(_n: number) {
  return function handler() {
    createEffect(count, () => {});
  };
}
export function FactoryHandler() {
  return <button onClick={makeHandler(0) as any} />;
}

// Negative: a wrapper around a member names the member's value, which is not
// the object's leading token, so the sugar stays silent as the bare member does.
const handlers = {
  onTap() {
    createEffect(count, () => {});
  },
};
export function MemberHandler() {
  return <button onClick={handlers.onTap as any} />;
}
