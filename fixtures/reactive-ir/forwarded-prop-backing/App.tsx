import { createSignal, createStore, merge } from "solid-js";

type Item = { text: string };

// Clean: `Plain` forwards a plain object and a string. The getters the
// compiler writes read no signal, so the body reads nothing reactive.
function PlainInner(props: { value: Item; label: string }) {
  const item = props.value;
  const label = props.label;
  return <div>{item.text}{label}</div>;
}
function PlainOuter(props: { value: Item; label: string }) {
  return <PlainInner value={props.value} label={props.label} />;
}

// Violation: `LiveOuter`'s only tag passes a signal read, so the forwarded
// prop is live in `LiveInner` and its body read is untracked.
function LiveInner(props: { count: number }) {
  const count = props.count;
  return <span>{count}</span>;
}
function LiveOuter(props: { count: number }) {
  return <LiveInner count={props.count} />;
}

// Uncertifiable: `EscapingOuter` is exported, so its callers are not all
// visible and the forwarded prop may be live.
function EscapingInner(props: { count: number }) {
  const count = props.count;
  return <span>{count}</span>;
}
export function EscapingOuter(props: { count: number }) {
  return <EscapingInner count={props.count} />;
}

// Uncertifiable: a whole props object passed on forwards every prop.
function WholeInner(props: { source: { count: number } }) {
  const source = props.source;
  return <span>{source.count}</span>;
}
function WholeOuter(props: { count: number }) {
  return <WholeInner source={props} />;
}

// Uncertifiable: `merge` with a getter override is a view, not the parent's
// props; its `value` is the override's.
function MergedInner(props: { value: number }) {
  const value = props.value;
  return <span>{value}</span>;
}
function MergedOuter(props: { value: number }) {
  const [count] = createSignal(1);
  const view = merge(props, {
    get value() {
      return count();
    },
  });
  return <MergedInner value={view.value} />;
}

// Uncertifiable: a static head says nothing about `.b`, a getter that reads
// a signal.
function SuffixInner(props: { value: number }) {
  const value = props.value;
  return <span>{value}</span>;
}
function SuffixOuter(props: { box: { b: number } }) {
  return <SuffixInner value={props.box.b} />;
}

// Uncertifiable: a store passed whole is a reference; reading the prop reads
// no key.
function StoreInner(props: { state: { count: number } }) {
  const state = props.state;
  return <span>{state.count}</span>;
}

// Uncertifiable: several children compile to memo accessors.
function ChildrenInner(props: { children?: unknown }) {
  const captured = props.children;
  return <div>{String(captured !== undefined)}</div>;
}
function ChildrenOuter(props: { count: number }) {
  return (
    <ChildrenInner>
      {props.count}
      {"tail"}
    </ChildrenInner>
  );
}

export function App() {
  const [count] = createSignal(1);
  const [store] = createStore({ count: 1 });
  return (
    <div>
      <PlainOuter value={{ text: "plain" }} label="static" />
      <LiveOuter count={count()} />
      <WholeOuter count={count()} />
      <MergedOuter value={0} />
      <SuffixOuter
        box={{
          get b() {
            return count();
          },
        }}
      />
      <StoreInner state={store} />
      <ChildrenOuter count={count()} />
    </div>
  );
}
