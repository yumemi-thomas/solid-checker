import { createRenderEffect } from "solid-js";
import { remote } from "./source";

// Uncertifiable (ADR 0510 review): the body reads a member of an `any` before
// the effect is created; `props.cfg` is undefined, so the body throws first.
export function Module(props: { cfg?: any }) {
  const value = props.cfg.x;
  createRenderEffect(
    () => 1,
    () => {
      remote();
    },
  );
  return <p>{String(value)}</p>;
}
