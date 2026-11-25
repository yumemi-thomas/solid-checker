import { createEffect, createOptimistic, createRenderEffect, onSettled } from "solid-js";

// ADR 1224: each export is the matching reactive-ir fixture `optimistic-effect-first-run(-render, -reads, -wrapped, -settled, -named)`, its
// child renamed. Chrome throws OPTIMISTIC_WRITE_BEFORE_LOAD in `Effect`, `Render`, `Settled` and `Named`; the other two stay uncertifiable.

function KidEffect(props: { run: () => void }) {
  createEffect(
    () => 1,
    () => {
      props.run();
    },
  );
  return <p />;
}

export function Effect() {
  const [, setO] = createOptimistic(async () => 1);
  return (
    <div>
      <KidEffect run={() => setO(2)} />
    </div>
  );
}

function KidRender(props: { run: () => void }) {
  createRenderEffect(
    () => 1,
    () => {
      props.run();
    },
  );
  return <p />;
}

export function Render() {
  const [, setO] = createOptimistic(async () => 1);
  return (
    <div>
      <KidRender run={() => setO(3)} />
    </div>
  );
}

function KidReads(props: { run: () => void }) {
  createEffect(
    () => 1,
    () => {
      props.run();
    },
  );
  return <p />;
}

export function Reads() {
  const [o, setO] = createOptimistic(async () => 1);
  return (
    <div>
      <KidReads run={() => setO(4)} />
      <p>{o()}</p>
    </div>
  );
}

function KidWrapped(props: { run: () => void }) {
  createEffect(
    () => 1,
    () => {
      props.run();
    },
  );
  return <p />;
}

function WrapWrapped(props: { children?: unknown }) {
  return <div>{props.children as never}</div>;
}

export function Wrapped() {
  const [, setO] = createOptimistic(async () => 1);
  return (
    <WrapWrapped>
      <KidWrapped run={() => setO(5)} />
    </WrapWrapped>
  );
}

function KidSettled(props: { run: () => void }) {
  onSettled(() => {
    props.run();
  });
  return <p />;
}

export function Settled() {
  const [, setO] = createOptimistic(async () => 1);
  return (
    <div>
      <KidSettled run={() => setO(6)} />
    </div>
  );
}

function KidNamed(props: { run: () => void }) {
  createEffect(
    () => 1,
    () => {
      props.run();
    },
  );
  return <p />;
}

export function Named() {
  const [, setO] = createOptimistic(async () => 1);
  function run() {
    setO(7);
  }
  return (
    <div>
      <KidNamed run={run} />
    </div>
  );
}
