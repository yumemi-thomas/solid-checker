import { createRenderEffect } from "solid-js";

// The effect a host-dispatched handler creates: no owner is live when the event
// fires, so nothing disposes it (NO_OWNER_EFFECT at runtime).
function listen(type: string) {
  createRenderEffect(
    () => type,
    (value) => {
      console.log(value);
    },
  );
}

// `handlers.onPointerDown` names the literal's function exactly.
export function Member() {
  const handlers = {
    onPointerDown: () => {
      listen("pointermove");
    },
  };
  return <button onPointerDown={handlers.onPointerDown} />;
}

// A spread of a const object literal onto an intrinsic element.
export function Spread() {
  const handlers = {
    onPointerDown: () => {
      listen("pointermove");
    },
  };
  return <button {...handlers} />;
}

// A spread of an object literal written in place.
export function SpreadLiteral() {
  return (
    <button
      {...{
        onPointerDown: () => {
          listen("pointermove");
        },
      }}
    />
  );
}

// Spread runtime installs lowercase on-keys.
export function Lowercase() {
  return <button {...{ title: "lowercase", onpointerdown: () => {
    createRenderEffect(() => "lowercase", () => {});
  } }} />;
}

// A namespace does not install a listener; invoking a generator runs no body.
export function InertControls() {
  (window as any).__handlerBodyRuns = 0;
  const handlers = { onClick: function* () {
    (window as any).__handlerBodyRuns++;
    createRenderEffect(() => "generator", () => {});
  } };
  return <><button data-probe="generator" onClick={handlers.onClick} />
    <button {...{ title: "namespaced", "on:click": () => {
      (window as any).__handlerBodyRuns++;
      createRenderEffect(() => "namespace", () => {});
    } }} /></>;
}
