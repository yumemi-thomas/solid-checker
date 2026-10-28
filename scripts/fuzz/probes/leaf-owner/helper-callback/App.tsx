// ADR 0492 (from the misuse matrix): a primitive created in a callback a project helper calls in place, inside createTrackedEffect, is in a forbidden scope (PRIMITIVE_IN_FORBIDDEN_SCOPE).
import { createMemo, createSignal, createTrackedEffect } from "solid-js";

function h1(cb: () => unknown) {
  cb();
  return 1;
}

export function C1() {
  createTrackedEffect(() => {
    h1(() => {
      createMemo(() => 1);
    });
  });
  return (
    <div>
    </div>
  );
}
