import type { Component } from "solid-js";

const STATIC = false;

export const StaticGuard: Component = () =>
  STATIC ? <div>Debug</div> : <div>Ready</div>;

export const SingleReturn: Component<{ ready: boolean }> = props => (
  <div>{props.ready ? "Ready" : "Waiting"}</div>
);

export function helper(value: boolean) {
  if (value) return <div>Yes</div>;
  return <div>No</div>;
}

export const UnrelatedReactiveCondition: Component<{ ready: boolean }> = props => {
  if (STATIC) return <div>Debug</div>;
  const label = props.ready ? "Ready" : "Waiting";
  return <div>{label}</div>;
};
