import type { Component } from "solid-js";

export const Alias: Component<{ name: string }> = props => {
  const alias = props;
  const name = alias.name;
  return <h1>{name}</h1>;
};

export const Computed: Component<Record<string, string>> = props => {
  const key = "name";
  const value = props[key];
  return <h1>{value}</h1>;
};

export const Spread: Component<{ name: string }> = props => {
  const copy = { ...props };
  return <h1>{copy.name}</h1>;
};

export const Destructure: Component<{ name: string }> = ({ name }) => (
  <h1>{name}</h1>
);
