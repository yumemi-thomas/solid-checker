import { createOptimistic } from "solid-js";

// ADR 1216 part 2: SC2009 claims the setter runs before the first answer, so the write, and every call that carries it
// there, must run whenever its function runs: a completion site (no branch, loop, `try`, logical or earlier exit
// around it), in no JSX element but an intrinsic one.

// Violations: OPTIMISTIC_WRITE_BEFORE_LOAD on rc.21.
export function Straight() {
  const [value, setValue] = createOptimistic(async () => 1);
  const write = () => {
    setValue(2);
  };
  write();
  return <p>{value()}</p>;
}

export function IntrinsicJsx() {
  const [value, setValue] = createOptimistic(async () => 1);
  return <p>{value()}<i>{(setValue(2), "x")}</i></p>;
}

// Uncertifiable: no proof that the write runs (main: violations Chrome does not confirm).
export function AbsentProp(props: { go?: boolean }) {
  const [value, setValue] = createOptimistic(async () => 1);
  const write = () => {
    setValue(3);
  };
  if (props.go) write();
  return <p>{value()}</p>;
}

function Child(props: { go: boolean }) {
  const [value, setValue] = createOptimistic(async () => 1);
  const write = () => {
    setValue(4);
  };
  if (props.go) write();
  return <p>{value()}</p>;
}
export function PassedFalse() {
  return <Child go={false} />;
}

export function RuntimeFalse() {
  const [value, setValue] = createOptimistic(async () => 1);
  const write = () => {
    setValue(5);
  };
  const ok = Math.random() > 2;
  if (ok) write();
  return <p>{value()}</p>;
}

export function DirectUnderBranch(props: { go?: boolean }) {
  const [value, setValue] = createOptimistic(async () => 1);
  if (props.go) setValue(6);
  return <p>{value()}</p>;
}

export function EarlyReturn(props: { stop?: boolean }) {
  const [value, setValue] = createOptimistic(async () => 1);
  if (!props.stop) return <p>{value()}</p>;
  setValue(7);
  return <p>{value()}</p>;
}

function Box(props: { children?: unknown }) {
  return <p>{props.children ? "x" : "y"}</p>;
}
export function ComponentChildren() {
  const [value, setValue] = createOptimistic(async () => 1);
  return <p>{value()}<Box>{(setValue(8), "x")}</Box></p>;
}
