import type { Component } from "solid-js";

// Baseline: an unwrapped component. Its name resolves directly, so it is
// classified as a component and destructuring its props is reported.
export const Plain: Component<{ name: string }> = ({ name }) => <h1>{name}</h1>;
