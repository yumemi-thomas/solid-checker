import { createEffect, runWithOwner, type Owner } from "solid-js";

runWithOwner(null, () => {
  createEffect(() => 1, () => {});
});

declare const definiteOwner: Owner;
runWithOwner(definiteOwner, () => {
  createEffect(() => 1, () => {});
});

declare const nullableOwner: Owner | null;
runWithOwner(nullableOwner, () => {
  createEffect(() => 1, () => {});
});

type AliasedNullableOwner = Owner | null;
declare const aliasedNullableOwner: AliasedNullableOwner;
runWithOwner(aliasedNullableOwner, () => {
  createEffect(() => 1, () => {});
});
