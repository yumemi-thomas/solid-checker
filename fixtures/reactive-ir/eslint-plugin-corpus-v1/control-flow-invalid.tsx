// Solid 1.x control-flow callbacks. This file was a byte copy of the 2.0
// corpus until the fixture's dialect stub was added: with no
// node_modules/solid-js/package.json it ran the 2.0 catalog, so two 2.0-only
// `For` shapes below typechecked against nothing and were analyzed by the
// wrong dialect. 1.x `For` has no `keyed` prop at all -- `keyed={false}` and
// `keyed={item => ...}` are TS2322 against the adjacent 1.x declarations --
// so the accessor-item case is spelled the way 1.x spells it, with <Index>,
// and custom keying has no 1.x equivalent and is covered only by the 2.0
// corpus.
import {
  For,
  Index,
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

// 1.x's accessor-item control flow: <Index> hands the callback an
// `Accessor<T>` item and a plain `number` index, the mirror image of <For>.
export function IndexItem(props: { items: User[] }) {
  return <Index each={props.items}>{item => {
    const name = item().name;
    return <span>{name}</span>;
  }}</Index>;
}

export function SignalInShow(props: { visible: boolean }) {
  const [count] = createSignal(0);
  return <Show when={props.visible}>{() => {
    console.log(count());
    return <span>{count()}</span>;
  }}</Show>;
}
