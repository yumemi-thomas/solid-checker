import { createMemo, onSettled } from "solid-js";

// ADR 1190 (slice 3): the named (`run={run1}`) and destructured (`({ run }) => run()`) forms of a callback prop are entered by
// the child's calls as ADR 1171's inline literal is.

function SettledKid(props: { run: () => void }) {
  onSettled(() => {
    props.run();
  });
  return <p>x</p>;
}
// A function named nowhere but the attribute: a creation run in onSettled. SC3001 violation at the call.
export function NamedSettled() {
  function run1() {
    createMemo(() => 1);
  }
  return <SettledKid run={run1} />;
}

function BodyKid(props: { run: () => void }) {
  props.run();
  return <p>x</p>;
}
// The same named function called in the child's body: clean.
export function NamedBody() {
  const run1 = () => {
    createMemo(() => 1);
  };
  return <BodyKid run={run1} />;
}

// A destructured prop called in onSettled. SC3001 violation at the call.
function DestructuredKid({ run }: { run: () => void }) {
  onSettled(() => {
    run();
  });
  return <p>x</p>;
}
export function DestructuredSettled() {
  return (
    <DestructuredKid
      run={() => {
        createMemo(() => 1);
      }}
    />
  );
}

// A destructured prop called in the child's body: clean.
function DestructuredBodyKid({ run }: { run: () => void }) {
  run();
  return <p>x</p>;
}
export function DestructuredBody() {
  return (
    <DestructuredBodyKid
      run={() => {
        createMemo(() => 1);
      }}
    />
  );
}

// The name also called elsewhere: its runs are not only the child's calls. Rows kept.
function TwiceKid(props: { run: () => void }) {
  props.run();
  return <p>x</p>;
}
export function NamedTwice() {
  function run2() {
    createMemo(() => 1);
  }
  onSettled(() => run2());
  return <TwiceKid run={run2} />;
}

// A destructured prop with a default: the default may run instead. Rows kept.
function DefaultedKid({ run = () => {} }: { run?: () => void }) {
  onSettled(() => {
    run();
  });
  return <p>x</p>;
}
export function DestructuredDefault() {
  return (
    <DefaultedKid
      run={() => {
        createMemo(() => 1);
      }}
    />
  );
}
