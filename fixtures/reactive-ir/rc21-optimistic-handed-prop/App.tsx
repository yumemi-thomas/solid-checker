import { createOptimistic, createSignal } from "solid-js";

function K(props: { s: (value: number) => void }) {
  return <button onClick={() => props.s(2)}>x</button>;
}

// Handed as a prop value: K's `props.s(2)` may run before the first answer.
// Prop value flow is not followed, so each hand-off is an obligation.
export function Handed() {
  const [, setValue] = createOptimistic(async () => 1);
  return <K s={setValue} />;
}

export function HandedCast() {
  const [, setValue] = createOptimistic(async () => 1);
  return <K s={setValue as (value: number) => void} />;
}

// A sync source has its answer, and a signal setter never throws this.
export function Loaded() {
  const [, setValue] = createOptimistic(1);
  const [, setCount] = createSignal(0);
  return (
    <p>
      <K s={setValue} />
      <K s={setCount} />
    </p>
  );
}

// A function written in the prop is walked where it calls the setter.
export function Wrapped() {
  const [, setValue] = createOptimistic(async () => 1);
  return <K s={(value) => setValue(value)} />;
}

// A branch may hold the setter; a negation or a comparison holds no function.
export function Branches(props: { on: boolean }) {
  const [value, setValue] = createOptimistic(async () => 1);
  const noop = (_: number) => {};
  return (
    <p>
      <K s={props.on ? setValue : noop} />
      <F on={!value()} n={value() > 1} />
    </p>
  );
}

function F(props: { on: boolean; n: boolean }) {
  return <p>{props.on}{props.n}</p>;
}
