import { createSignal, Show } from "solid-js";

const [count] = createSignal(1);

export function Bad() {
  return <Show when={count()}>{() => {
    const frozen = count();
    return <span>{frozen}</span>;
  }}</Show>;
}

export function Good() {
  return <Show when={count()}>{() => <span>{count()}</span>}</Show>;
}

export function ParameterReads() {
  return <Show when={count()}>{value => {
    const frozen = value();
    return <span>{frozen}</span>;
  }}</Show>;
}

export function ParameterTracked() {
  return <Show when={count()}>{value => <span>{value()}</span>}</Show>;
}
