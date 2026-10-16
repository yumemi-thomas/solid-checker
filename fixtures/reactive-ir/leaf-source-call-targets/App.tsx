import { createEffect, createTrackedEffect } from "solid-js";
import { makeArrowService, makeSelection, makeService } from "./service";

export function CleanMethod() {
  const service = makeService();
  createTrackedEffect(() => { service.cleanLeaf(); });
  return <div />;
}
export function CleanArrow() {
  const service = makeArrowService();
  createTrackedEffect(() => { service.arrowLeaf(); });
  return <div />;
}
export function ForbiddenMethod() {
  const service = makeService();
  createTrackedEffect(() => { service.forbiddenLeaf(); });
  return <div />;
}
export function NestedPrimitive() {
  const service = makeService();
  createTrackedEffect(() => { service.nestedLeaf(); });
  return <div />;
}
export function ConditionalMethod(props: { flag: boolean }) {
  const service = makeService();
  createTrackedEffect(() => { service.conditionalLeaf(props.flag); });
  return <div />;
}
export function DefaultMethod() {
  const service = makeService();
  createTrackedEffect(() => { service.defaultLeaf(); });
  return <div />;
}
// Mirrors the newly exposed cluster.selected() read in the corpus.
export function SelectedInApply() {
  const cluster = makeSelection();
  createEffect(() => 1, (next) => {
    if (next !== cluster.selected()) cluster.select(next);
  });
  return <div />;
}
