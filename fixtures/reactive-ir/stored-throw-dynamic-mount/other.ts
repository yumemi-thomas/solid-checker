import type { Component } from "solid-js";

// A project function named `dynamic`: no dialect primitive, whatever it does.
export function dynamic<T>(source: () => T): Component<{}> {
  return () => {
    source();
    return null;
  };
}
