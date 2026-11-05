import { createSignal } from "solid-js";

export const Anonymous = [
  function () {
    const [count] = createSignal(1);
    return count() ? <a /> : <b />;
  },
][0]!;

export function Named() {
  const [count] = createSignal(1);
  return count() ? <a /> : <b />;
}

export const Quiet = [
  function () {
    const [count] = createSignal(1);
    return <a>{count()}</a>;
  },
][0]!;
