import { createTrackedEffect } from "solid-js";

type Program = { blend?: number };

// Positive control: a write through the props object itself is dropped.
export function WriteThroughProps(props: { count: number }) {
  props.count = 2;
  return <div />;
}

// Positive control: a whole-object alias is still the props container.
export function WriteThroughAlias(props: { count: number }) {
  const alias = props;
  alias.count = 3;
  return <div />;
}

// Negative: `program` is the VALUE of one property, an application-owned
// mutable object. Writing to it is not a write to the readonly props proxy.
export function WriteThroughDestructuredValue(props: { program?: Program; blend?: number }) {
  const { program } = props;
  createTrackedEffect(() => {
    if (props.blend && program) {
      program.blend = props.blend;
    }
  });
  return <div />;
}
