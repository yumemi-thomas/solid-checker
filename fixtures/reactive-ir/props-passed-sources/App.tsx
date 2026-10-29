import { action, createOptimistic, createSignal } from "solid-js";
import type { Accessor, Setter } from "solid-js";

// Positive: the parent passes its own signal's accessor and setter, and an
// action; the private child calls them in its body. A static prop read is
// silent, but calling the value reads, writes or acts with the parent's
// source (SC1001, SC2001, the action violation).
type Props = { count: Accessor<number>; setCount: Setter<number>; save: () => Promise<void> };

function Member(props: Props) {
  props.count();
  props.setCount(1);
  props.save();
  return <p />;
}

function Destructured({ count, setCount }: Props) {
  count();
  setCount(2);
  return <p />;
}

// Negative: in a handler the write is legal.
function Handler(props: Props) {
  return <button onClick={() => props.setCount(3)} />;
}

// Negative: an optimistic setter meets no owned-scope guard on rc.17.
function Optimistic(props: { setCount: Setter<number> }) {
  props.setCount(4);
  return <p />;
}

// Uncertifiable: the value is an arrow the parent writes, which this does not
// follow; never certified clean.
function Wrapped(props: { onChange: (value: number) => void }) {
  props.onChange(5);
  return <p />;
}

export function Parent() {
  const [count, setCount] = createSignal(0);
  const [, setOptimistic] = createOptimistic(0);
  const save = action(function* () {});
  return (
    <p>
      <Member count={count} setCount={setCount} save={save} />
      <Destructured count={count} setCount={setCount} save={save} />
      <Handler count={count} setCount={setCount} save={save} />
      <Optimistic setCount={setOptimistic} />
      <Wrapped onChange={(value) => setCount(value)} />
    </p>
  );
}
