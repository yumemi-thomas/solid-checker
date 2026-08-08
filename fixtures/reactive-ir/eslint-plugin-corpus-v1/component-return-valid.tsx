import type { Component } from "solid-js";

export const SingleReturn: Component<{ ready: boolean }> = props => (
  <div>{props.ready ? "Ready" : "Waiting"}</div>
);

// Not a component: a lowercase helper may return from several branches.
export function helper(value: boolean) {
  if (value) return <div>Yes</div>;
  return <div>No</div>;
}
