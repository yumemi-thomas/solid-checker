import { createSignal } from "solid-js";

const [remote] = createSignal(1);

export async function remotePrefix() {
  const value = remote();
  await Promise.resolve();
  return value;
}

export async function remoteSuffix() {
  await Promise.resolve();
  return remote();
}
