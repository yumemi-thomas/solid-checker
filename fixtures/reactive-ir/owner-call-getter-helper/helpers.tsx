import { createMemo } from "solid-js";

function Tag() {
  return <p>tag</p>;
}

// Written in another file: the entry walk follows a call of an imported function to its caller's file.
export function sharedView() {
  return <Tag />;
}

export function sharedMemo() {
  createMemo(() => 1);
  return 1;
}
