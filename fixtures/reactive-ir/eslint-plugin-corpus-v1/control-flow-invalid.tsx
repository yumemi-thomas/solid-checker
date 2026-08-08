import {
  For,
  Show,
  createSignal,
} from "solid-js";

type User = { name: string };

export function PropsInShow(props: {
  visible: boolean;
  name: string;
}) {
  return <Show when={props.visible}>{() => {
    const name = props.name;
    return <span>{name}</span>;
  }}</Show>;
}

export function ShowAccessor(props: { user?: User }) {
  return <Show when={props.user}>{user => {
    const name = user().name;
    return <span>{name}</span>;
  }}</Show>;
}

export function ForIndex(props: { items: User[] }) {
  return <For each={props.items}>{(item, index) => {
    const position = index();
    return <span>{position}: {item.name}</span>;
  }}</For>;
}

export function UnkeyedForItem(props: { items: User[] }) {
  return <For each={props.items} keyed={false}>{item => {
    const name = item().name;
    return <span>{name}</span>;
  }}</For>;
}

export function CustomKeyedFor(props: { items: User[] }) {
  return <For each={props.items} keyed={item => item.name}>{(item, index) => {
    const name = item().name;
    const position = index();
    return <span>{position}: {name}</span>;
  }}</For>;
}

export function SignalInShow(props: { visible: boolean }) {
  const [count] = createSignal(0);
  return <Show when={props.visible}>{() => {
    console.log(count());
    return <span>{count()}</span>;
  }}</Show>;
}
