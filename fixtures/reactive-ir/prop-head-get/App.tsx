import { createEffect, createSignal, createStore, For, merge, Show, untrack } from "solid-js";
import * as Solid from "solid-js";
import { Forwarder } from "./Forwarder";
import type { Accessor, Store } from "solid-js";

type Item = { text: string };
type Props = { value: Item; live: Item; selector: Item };

// CLEAN: static one-link forwards remain static at any depth. No root alias.
function PlainInner(props: { value: Item }) {
  return <For each={[0]}>{(_row) => {
    const text = props.value.text;
    return <p>{text}</p>;
  }}</For>;
}
function PlainMiddle(props: { value: Item }) { return <PlainInner value={props.value} />; }
function PlainOuter(props: { value: Item }) { return <PlainMiddle value={props.value} />; }
function TypedInner(props: { value: Item }) {
  const text = props.value.text; // Unresolved, never proven from a type annotation.
  return <p>{text}</p>;
}
function TypedOuter(props: { value: Item }) { return <TypedInner value={props.value} />; }

// NO NEW VIOLATION: the forwarding root no longer denotes its incoming props.
function ReplacedForwardInner(props: { value: Item }) {
  const text = props.value.text;
  return <p>{text}</p>;
}
function ReplacedForwardOuter(props: { value: Item }) {
  props = { value: { text: "plain" } };
  return <ReplacedForwardInner value={props.value} />;
}

// NO NEW VIOLATION: store protocol/symbol Gets aren't strict-read witnesses.
function ThenInner(props: { value: Item }) {
  const text = props.value.text;
  return <p>{text}</p>;
}
function ThenOuter(props: { value: Item }) { return <ThenInner value={props.value} />; }
function SymbolInner(props: { value: Item }) {
  const text = props.value.text;
  return <p>{text}</p>;
}
function SymbolOuter(props: { value: Item }) { return <SymbolInner value={props.value} />; }
function DynamicStoreInner(props: { value: Item }) {
  const text = props.value.text;
  return <p>{text}</p>;
}
function DynamicStoreOuter(props: { value: Item }) { return <DynamicStoreInner value={props.value} />; }

// CLEAN: static function lookup is not a reactive read or an invocation proof.
function StableFunction(props: { onClick: () => void; ref: () => void }) {
  const bound = props.onClick.bind(null);
  props["ref"]();
  return <button onClick={bound}>plain callback</button>;
}

// VIOLATION at the cooked value Get, not at .text, with authored key spelling.
// The absent dotted literal key stays static; it is not the separate key "a".
function Cooked(props: { value: Item; "a.b"?: Item }) {
  const first = (props as typeof props)["val\u0075e"].text;
  const second = props["a.b"]?.text;
  return <p>{first}{second}</p>;
}
// UNCERTIFIABLE: spelling "selector" names a live sibling, but selects plain value.
function Dynamic(props: Props) {
  const selector: "value" | "live" = "value";
  const item = props[selector].text;
  return <p>{item}</p>;
}

// CLEAN: JSX/tracked/sampled reads do not become For-body reads by capture.
function CapturedCorrect(props: { value: Item }) {
  return <For each={[0]}>{(_row) => {
    const later = () => props.value.text;
    const sampled = untrack(() => props.value.text);
    createEffect(() => props.value.text, () => {});
    return <button onClick={() => { void props.value.text; }}>{later()}{sampled}</button>;
  }}</For>;
}
// NO NEW VIOLATION: nested default isn't run when its function is constructed.
function DormantDefault(props: { value: Item }) {
  return <For each={[0]}>{(_row) => {
    function later(value = props.value.text) { return value; }
    void later;
    return <p>idle</p>;
  }}</For>;
}

// VIOLATION: namespace import resolves the actual dialect primitive.
function NamespaceChild(props: { value: Item }) {
  return <Solid.For each={[0]}>{(_row) => {
    const text = props.value.text;
    return <p>{text}</p>;
  }}</Solid.For>;
}
// VIOLATION: keyed Show supplies a plain value; captured prop backing is separate.
function KeyedShow(props: { value: Item }) {
  return <Show when={{ visible: true }} keyed>{(_shown) => {
    const text = props.value.text;
    return <p>{text}</p>;
  }}</Show>;
}
// NO NEW VIOLATION: identical spelling does not make this local For a primitive.
function Shadowed(props: { value: Item }) {
  const For = (input: { each: number[]; children: (row: number) => unknown }) => {
    void input;
    return <p>callback not invoked</p>;
  };
  return <For each={[0]}>{(_row) => {
    const text = props.value.text;
    return <p>{text}</p>;
  }}</For>;
}
// NO NEW VIOLATION: an unknown wrapper's result, not its literal argument, is child.
declare function replaceChild(fn: (row: number) => ReturnType<typeof Show>):
  (row: number) => ReturnType<typeof Show>;
function WrappedChild(props: { value: Item }) {
  return <For each={[0]}>{replaceChild((_row) => {
    const text = props.value.text;
    return <p>{text}</p>;
  })}</For>;
}

// UNCERTIFIABLE: original incoming static prop says nothing about a merge view.
function MergedHead(props: { value: Item }) {
  const [live] = createSignal({ text: "live" });
  const view = merge(props, { get value() { return live(); } });
  const text = view.value.text;
  return <p>{text}</p>;
}
// UNCERTIFIABLE: aliases are not certified as their original parameter's keys.
function AliasedHead(props: { value: Item }) {
  const alias = props;
  const text = alias.value.text;
  return <p>{text}</p>;
}
// NO VIOLATION: an incoming live witness cannot describe a replaced root.
function ReplacedRoot(props: { value: Item }) {
  props = { value: { text: "plain" } };
  const text = props.value.text;
  return <p>{text}</p>;
}
// UNCERTIFIABLE: replacement of a property withdraws its incoming witness.
function ReplacedProperty(props: { value: Item }) {
  props.value = { text: "plain" };
  const text = props.value.text;
  return <p>{text}</p>;
}

// UNCERTIFIABLE (ADR 0216): a static head doesn't certify an arbitrary suffix.
function SuffixInner(props: { value: Item }) {
  const text = props.value.text;
  return <p>{text}</p>;
}
function SuffixOuter(props: { box: { value: Item } }) {
  return <SuffixInner value={props.box.value} />;
}
// UNCERTIFIABLE (ADR 0216): whole objects/whole stores aren't reactive Get proofs.
function WholeInner(props: { source: { value: Item } }) {
  const text = props.source.value.text;
  return <p>{text}</p>;
}
function WholeProps(props: { value: Item }) { return <WholeInner source={props} />; }

// UNCERTIFIABLE (ADR 0216): children array contains memo accessors, not direct Gets.
function ChildrenInner(props: { children?: unknown }) {
  const children = props.children;
  return <p>{String(children !== undefined)}</p>;
}
function SeveralChildren(props: { value: Item }) {
  return <ChildrenInner>{props.value.text}{"tail"}</ChildrenInner>;
}

// UNCERTIFIABLE: exported upstream has no visible live witness.
function EscapingInner(props: { value: Item }) {
  const text = props.value.text;
  return <p>{text}</p>;
}
export function EscapingOuter(props: { value: Item }) {
  return <EscapingInner value={props.value} />;
}

export default function App() {
  const [live] = createSignal({ text: "live" });
  const [store] = createStore({ value: { text: "store" } });
  const plain = { text: "plain" };
  const stable = () => {};
  const typedPlain: Accessor<Item> = () => plain;
  const typedStore: Store<{ value: Item }> = { value: plain };
  const marker = Symbol("plain");
  const [protocolStore] = createStore({ then: plain, value: plain, [marker]: plain });
  const protocolKey: "then" | "value" = "then";
  return <>
    <PlainOuter value={plain} />
    <TypedOuter value={typedPlain()} />
    <TypedOuter value={typedStore.value} />
    <ReplacedForwardOuter value={live()} />
    <ThenOuter value={protocolStore.then} />
    <SymbolOuter value={protocolStore[marker]} />
    <DynamicStoreOuter value={protocolStore[protocolKey]} />
    <StableFunction onClick={stable} ref={stable} />
    <Cooked value={live()} />
    <Dynamic value={plain} live={live()} selector={live()} />
    <CapturedCorrect value={live()} />
    <DormantDefault value={live()} />
    <NamespaceChild value={live()} />
    <KeyedShow value={live()} />
    <Shadowed value={live()} />
    <WrappedChild value={live()} />
    <MergedHead value={plain} />
    <AliasedHead value={plain} />
    <ReplacedRoot value={live()} />
    <ReplacedProperty value={live()} />
    <SuffixOuter box={{ get value() { return live(); } }} />
    <WholeProps value={live()} />
    <WholeInner source={store} />
    <SeveralChildren value={live()} />
    <Forwarder value={live()} />
  </>;
}
