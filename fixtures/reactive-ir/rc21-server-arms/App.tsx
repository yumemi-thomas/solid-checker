import { createSignal, createStore } from "solid-js";
import * as Web from "@solidjs/web";
import { isDev, isServer } from "@solidjs/web";

const onServer = isServer;

export function Arms() {
  const [count] = createSignal(0);
  const [store] = createStore({ a: 1 });
  if (isServer) {
    store.a = 2;
    count();
  }
  if (!isServer) {
    count();
  } else {
    count();
  }
  if (onServer) count();
  if (Web.isServer) count();
  if (isServer === true) count();
  const picked = isServer ? count() : 0;
  const guarded = isServer && count();
  return <p>{picked}{guarded}</p>;
}

export function EarlyReturn() {
  const [count] = createSignal(0);
  if (!isServer) return <p />;
  count();
  return <p />;
}

export function Constants() {
  const [count] = createSignal(0);
  if (typeof window === "undefined") count();
  if (isDev) count();
  if (process.env.NODE_ENV === "production") count();
  return <p />;
}

// A plain guard says nothing about the build.
export function Plain(props: { on: boolean }) {
  const [count] = createSignal(0);
  if (props.on) count();
  return <p />;
}
