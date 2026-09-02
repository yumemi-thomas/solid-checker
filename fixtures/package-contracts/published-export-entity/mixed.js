/// <reference path="./globals.d.ts" />

/** @type {(() => void) | { readonly kind: "value" }} */
export let mixedBinding = () => {};

if (globalThis.phase21MixedExport) mixedBinding = { kind: "value" };
