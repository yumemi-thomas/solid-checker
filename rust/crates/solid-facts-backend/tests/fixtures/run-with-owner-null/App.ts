import { createEffect, runWithOwner, type Owner as SolidOwner } from "solid-js";
import type { ReExportedOwner } from "./OwnerBridge";

runWithOwner(null, () => {
  createEffect(() => 1, () => {});
});

declare const definiteOwner: SolidOwner;
runWithOwner(definiteOwner, () => {
  createEffect(() => 1, () => {});
});

declare const nullableOwner: Owner | null;
runWithOwner(nullableOwner, () => {
  createEffect(() => 1, () => {});
});

type AliasedNullableOwner = SolidOwner | null;
declare const aliasedNullableOwner: AliasedNullableOwner;
runWithOwner(aliasedNullableOwner, () => {
  createEffect(() => 1, () => {});
});

declare const reExportedOwner: ReExportedOwner;
runWithOwner(reExportedOwner, () => {
  createEffect(() => 1, () => {});
});

type Owner = { readonly local: true };
declare const localOwner: Owner;
runWithOwner(localOwner, () => {
  createEffect(() => 1, () => {});
});

declare const unresolvedOwner: unknown;
runWithOwner(unresolvedOwner, () => {
  createEffect(() => 1, () => {});
});
