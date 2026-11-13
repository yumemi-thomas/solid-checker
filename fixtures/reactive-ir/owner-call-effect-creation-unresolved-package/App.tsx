import { createEffect, createMemo } from "solid-js";
import { render } from "@solidjs/web";

// Batch 11, slice 6 (ADR 0969): a creation the dialect says throws on an effect's apply, where the callee's package root is unknown, is no longer silent. The walk that proves the
// throw (`ownerless_effect_phase_origin`) needs the package the callee is declared in; with none it files nothing, and before the owner-call universe nothing else said so (the
// program below was certified). Now the creation in the apply ends in nothing the checker can name and gets `effect-creation-unaccounted` (SC6002, uncertifiable), never a violation:
// the same program with the declarations in an installed package (`owner-call-effect-creation`, `InApply`) is the walk's violation.
export function InApply() {
  createEffect(
    () => 0,
    () => {
      createMemo(() => 1);
    },
  );
  return <p>apply</p>;
}

render(() => <InApply />, document.body);
