import { createEffect, onCleanup } from "solid-js";

// ADR 1171: a component prop's function literal whose every run is a call `props.run()` the child writes, the child rendered
// at this one tag and nowhere else, runs where each call is written. The owner graph takes an edge from each call.

// Called in the child's body: the child's render owner is current, so `onCleanup` has an owner (silent).
function BodyKid(props: { run: () => void }) {
  props.run();
  return <p>body</p>;
}
export function Body() {
  return (
    <BodyKid
      run={() => {
        onCleanup(() => {});
      }}
    />
  );
}

// Called in an effect's apply: no owner is current there, so `onCleanup` is NO_OWNER_CLEANUP (SC4001).
function ApplyKid(props: { run: () => void }) {
  createEffect(
    () => 0,
    () => {
      props.run();
    },
  );
  return <p>apply</p>;
}
export function Apply() {
  return (
    <ApplyKid
      run={() => {
        onCleanup(() => {});
      }}
    />
  );
}

// Never called: no run to place it by; the row stands (a dead site is uncertifiable, ADR 1018).
function NeverKid(props: { run: () => void }) {
  return <p>{typeof props}</p>;
}
export function Never() {
  return (
    <NeverKid
      run={() => {
        onCleanup(() => {});
      }}
    />
  );
}

// Rendered at two tags: `props.run` holds either literal, and each call runs one of them; no edge.
function TwiceKid(props: { run: () => void }) {
  props.run();
  return <p>twice</p>;
}
export function Twice() {
  return (
    <>
      <TwiceKid
        run={() => {
          onCleanup(() => {});
        }}
      />
      <TwiceKid run={() => {}} />
    </>
  );
}

// `props.run` read as a value: the alias may run anywhere; no edge.
function HeldKid(props: { run: () => void }) {
  const held = props.run;
  setTimeout(held, 0);
  return <p>held</p>;
}
export function Held() {
  return (
    <HeldKid
      run={() => {
        onCleanup(() => {});
      }}
    />
  );
}

// A spread on the element may supply `run` under its key; no edge.
const extra: { run?: () => void } = {};
function SpreadKid(props: { run: () => void }) {
  props.run();
  return <p>spread</p>;
}
export function Spread() {
  return (
    <SpreadKid
      run={() => {
        onCleanup(() => {});
      }}
      {...extra}
    />
  );
}
