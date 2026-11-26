import { createOptimistic, createSignal } from "solid-js";

// ADR 1216 slice 2: the creating pass runs a callback prop where the child calls it in its body (ADR 1171, 1190) and a
// component where the tree renders it eagerly: a direct child of intrinsic elements. OPTIMISTIC_WRITE_BEFORE_LOAD on rc.21.

// Violations.
function Kid(props: { run: () => void }) {
  props.run();
  return <p>k</p>;
}
export function CallbackProp() {
  const [value, setValue] = createOptimistic(async () => 1);
  return (
    <div>
      <Kid run={() => setValue(2)} />
      <p>{value()}</p>
    </div>
  );
}

export function RenderedChild() {
  const [value, setValue] = createOptimistic(async () => 1);
  function Child() {
    setValue(3);
    return <i />;
  }
  return (
    <div>
      <Child />
      <p>{value()}</p>
    </div>
  );
}

function Leaf(props: { run: () => void }) {
  props.run();
  return <p>l</p>;
}
function Forward(props: { run: () => void }) {
  return <Leaf {...props} />;
}
export function ForwardedProp() {
  const [value, setValue] = createOptimistic(async () => 1);
  return (
    <div>
      <Forward run={() => setValue(4)} />
      <p>{value()}</p>
    </div>
  );
}

// Uncertifiable: no proven run on the creating pass.
export function ConditionalRender() {
  const [value, setValue] = createOptimistic(async () => 1);
  const [on, setOn] = createSignal(false);
  function Late() {
    setValue(5);
    return <i />;
  }
  return (
    <div>
      <button onClick={() => setOn(true)}>go</button>
      <p>{on() ? <Late /> : null}</p>
      <p>{value()}</p>
    </div>
  );
}

function ClickKid(props: { run: () => void }) {
  return <button onClick={() => props.run()}>k</button>;
}
export function PropInHandler() {
  const [value, setValue] = createOptimistic(async () => 1);
  return (
    <div>
      <ClickKid run={() => setValue(6)} />
      <p>{value()}</p>
    </div>
  );
}

function Box(props: { children?: unknown }) {
  return <p>{props.children ? "x" : "y"}</p>;
}
export function InComponentChildren() {
  const [value, setValue] = createOptimistic(async () => 1);
  function Inner() {
    setValue(7);
    return <i />;
  }
  return (
    <div>
      <Box>
        <Inner />
      </Box>
      <p>{value()}</p>
    </div>
  );
}
