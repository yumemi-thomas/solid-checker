import { createEffect } from "solid-js";
import { Dynamic } from "@solidjs/web";
import { Badge, Panel, Reader, Ui, registry, useUi } from "dep-ui-context";
import { Pure, PureBadge, pureHook } from "dep-ui-pure";

function Layout(props: { comp: (props: { children?: any }) => any }) {
  return <props.comp>x</props.comp>;
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
