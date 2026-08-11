import { createEffect, onCleanup } from "solid-js";

const installOrphans = () => {
  createEffect(() => 1, () => {});
  onCleanup(() => {});
};
installOrphans();

export const Widget = () => {
  createEffect(() => 2, () => {});
  onCleanup(() => {});
  return null;
};

export const useOrphan = () => {
  createEffect(() => 3, () => {});
  onCleanup(() => {});
};
