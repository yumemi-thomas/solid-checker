import { createEffect } from "solid-js";
import { Dynamic } from "@solidjs/web";
import { Badge, Panel, Reader, Ui, registry, useUi } from "dep-ui-context";
import { Pure, PureBadge, pureHook } from "dep-ui-pure";

function Layout(props: { comp: (props: { children?: any }) => any }) {
  return <props.comp>x</props.comp>;
}
// Project helpers that run a function handed to them: the call that runs it is the entry.
function run(f: () => unknown) {
  return f();
}
function runOnce(f: () => unknown) {
  return f();
}
function runVia(f: () => unknown) {
  return runOnce(f);
}
function keep(f: () => unknown) {
  return () => f;
}

export function Handed() {
  createEffect(useUi, () => {});
  return <p>x</p>;
}
export function Dynamically() {
  return <Dynamic component={Panel}>x</Dynamic>;
}
export function Prop() {
  return <Layout comp={Panel} />;
}
export function Child() {
  return <div>{Badge}</div>;
}
export function Method() {
  new Reader().read();
  return <p>x</p>;
}
export function Getter() {
  registry.current;
  return <p>x</p>;
}
export function Member() {
  return <Ui.Panel>x</Ui.Panel>;
}

export function PureHanded() {
  createEffect(pureHook, () => {});
  return <p>x</p>;
}
export function PureChild() {
  return <div>{PureBadge}</div>;
}
export function PureDynamic() {
  return <Dynamic component={Pure}>x</Dynamic>;
}

export function Helper() {
  run(useUi);
  return <p>x</p>;
}
export function HelperVia() {
  runVia(useUi);
  return <p>x</p>;
}
export function HelperKeeps() {
  keep(useUi);
  return <p>x</p>;
}
export function PureHelper() {
  run(pureHook);
  return <p>x</p>;
}
