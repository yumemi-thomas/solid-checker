import { createMemo, createTrackedEffect, flush, onSettled } from "solid-js";

// ADR 1190: a component prop's function literal whose runs ADR 1171 proves are the child's calls of `props.run` is walked
// at those calls by the leaf walk, and no unread package runs it elsewhere.

// Called in the child's body, under its render's owner: a creation there is legal. Clean (main: SC3001/SC6002 uncertifiable).
function BodyKid(props: { run: () => void }) {
  props.run();
  return <p>x</p>;
}
export function Body() {
  return (
    <BodyKid
      run={() => {
        createMemo(() => 1);
      }}
    />
  );
}

// Called in an onSettled callback: a creation there throws (PRIMITIVE_IN_FORBIDDEN_SCOPE). SC3001 violation at the call.
function SettledKid(props: { run: () => void }) {
  onSettled(() => {
    props.run();
  });
  return <p>x</p>;
}
export function Settled() {
  return (
    <SettledKid
      run={() => {
        createMemo(() => 1);
      }}
    />
  );
}

// Called in a tracked effect: a flush there throws (FLUSH_IN_TRACKED_QUEUE_CALLBACK). SC3001 violation at the call.
function TrackedKid(props: { run: () => void }) {
  createTrackedEffect(() => {
    props.run();
  });
  return <p>x</p>;
}
export function Tracked() {
  return (
    <TrackedKid
      run={() => {
        flush();
      }}
    />
  );
}

// Never called by the child: the dead-site ruling keeps the row.
function NeverKid(_props: { run: () => void }) {
  return <p>x</p>;
}
export function Never() {
  return (
    <NeverKid
      run={() => {
        createMemo(() => 1);
      }}
    />
  );
}

// A spread on the element: ADR 1171 does not prove the literal's runs, the row stays.
function SpreadKid(props: { run: () => void }) {
  onSettled(() => {
    props.run();
  });
  return <p>x</p>;
}
export function Spread() {
  const rest = {};
  return (
    <SpreadKid
      {...rest}
      run={() => {
        createMemo(() => 1);
      }}
    />
  );
}
