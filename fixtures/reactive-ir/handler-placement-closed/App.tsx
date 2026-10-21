import type { JSX } from "@solidjs/web";
import { createRenderEffect, createSignal } from "solid-js";

const [count, setCount] = createSignal(0);

// Member read: `handlers.onPointerDown` names the literal's function exactly.
export function Member() {
  const handlers = {
    onPointerDown: () => {
      createRenderEffect(() => "pointermove", () => {});
    },
  };
  return <button onPointerDown={handlers.onPointerDown} />;
}

// Spread of a const object literal onto an intrinsic element.
export function Spread() {
  const handlers = {
    onPointerDown: () => {
      createRenderEffect(() => "pointermove", () => {});
    },
  };
  return <button {...handlers} />;
}

// Spread of an object literal written in place.
export function SpreadLiteral() {
  return (
    <button
      {...{
        onPointerDown: () => {
          createRenderEffect(() => "pointermove", () => {});
        },
      }}
    />
  );
}

// Negative: the same member read, but the handler only writes a signal, which an event may do.
export function Clean() {
  const handlers = {
    onPointerDown: () => {
      setCount(count() + 1);
    },
  };
  return <button onPointerDown={handlers.onPointerDown} />;
}

// Obligation: the handler is read from props, and this file holds no literal for it.
export function Unresolved(props: { onPointerDown: JSX.EventHandler<HTMLButtonElement, PointerEvent> }) {
  return <button onPointerDown={props.onPointerDown} />;
}

// Obligation: an intrinsic element spreads props whose handlers are not visible here.
export function UnresolvedSpread(props: { onPointerDown?: () => void }) {
  return <button {...props} />;
}

// Obligation: the object is reassigned, so its member is not the literal's function.
export function Reassigned() {
  let handlers = {
    onPointerDown: () => {
      createRenderEffect(() => "pointermove", () => {});
    },
  };
  handlers = { onPointerDown: () => {} };
  return <button onPointerDown={handlers.onPointerDown} />;
}

// rc.13 installs lowercase on-keys too.
export function LowercaseSpread() {
  return <button {...{ title: "lowercase", onpointerdown: () => createRenderEffect(() => "lowercase", () => {}) }} />;
}

// A namespace prevents event installation by spread.
export function NamespacedSpread() {
  return <button {...{ title: "namespace", "on:click": () => createRenderEffect(() => "namespace", () => {}) }} />;
}

// Tuple position zero holds the exact member function.
export function TupleMember() {
  const handlers = { onPointerDown: () => createRenderEffect(() => "tuple", () => {}) };
  return <button onPointerDown={[handlers.onPointerDown, 1]} />;
}

// Reading another property can replace the listener: no exact spread proof.
export function GetterSpread() {
  const handlers = {
    get title(): string { this.onClick = () => {}; return "getter"; },
    onClick: () => createRenderEffect(() => "getter", () => {}),
  };
  return <button {...handlers} />;
}

// Calling a generator creates an iterator without executing its body.
export function GeneratorMember() {
  const handlers = { onPointerDown: function* () { createRenderEffect(() => "generator", () => {}); } };
  return <button onPointerDown={handlers.onPointerDown} />;
}

export function UnknownTuple(props: { handler: () => void }) {
  return <button onPointerDown={[props.handler, 1]} />;
}
