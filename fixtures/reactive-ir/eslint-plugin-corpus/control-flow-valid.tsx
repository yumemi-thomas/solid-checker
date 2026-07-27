import {
  For,
  Repeat,
  Show,
  untrack,
} from "solid-js";

type User = { name: string };

export function ReturnedJsx(props: {
  visible: boolean;
  name: string;
}) {
  return <Show when={props.visible}>{() => <span>{props.name}</span>}</Show>;
}

export function DeferredClosure(props: { name: string }) {
  return <Repeat count={1}>{() => {
    const click = () => console.log(props.name);
    return <button onClick={click}>Open</button>;
  }}</Repeat>;
}

export function RawValues(props: { items: User[] }) {
  return <>
    <For each={props.items}>{item => {
      const name = item.name;
      return <span>{name}</span>;
    }}</For>
    <Repeat count={2}>{index => {
      const value = index + 1;
      return <span>{value}</span>;
    }}</Repeat>
  </>;
}

export function UnkeyedIndexIsRaw(props: { items: User[] }) {
  return <For each={props.items} keyed={false}>{(item, index) => {
    const position = index + 1;
    return <span>{position}: {item().name}</span>;
  }}</For>;
}

export function KeyedShow(props: { user?: User }) {
  return <Show when={props.user} keyed>{user => {
    const name = user.name;
    return <span>{name}</span>;
  }}</Show>;
}

export function ExplicitUntrack(props: { user?: User }) {
  return <Show when={props.user}>{user => {
    const name = untrack(() => user().name);
    return <span>{name}</span>;
  }}</Show>;
}
