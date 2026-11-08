import { createMemo, createSignal, Loading } from "solid-js";

const [route] = createSignal(0);
const data = createMemo(async () => route());
const reader = { read: (n: number) => data() ?? n };
const held = {
  get value() {
    return data();
  },
};
const keep = (source: () => unknown) => source();
const plain = (n: number) => n + 1;

export function Method() {
  return <Loading on={route()} fallback="x"><p>{reader.read(1)}</p></Loading>;
}

export function Getter() {
  return <Loading on={route()} fallback="x"><p>{held.value as number}</p></Loading>;
}

export function Handed() {
  return <Loading on={route()} fallback="x"><p>{keep(data) as number}</p></Loading>;
}

export function Plain() {
  return <Loading on={route()} fallback="x"><p>{plain(1)}</p></Loading>;
}
