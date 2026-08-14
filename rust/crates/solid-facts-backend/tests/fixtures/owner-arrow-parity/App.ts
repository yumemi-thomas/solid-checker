import { createEffect, onCleanup, type Component } from "solid-js";

const installOrphans = () => {
  createEffect(() => 1, () => {});
  onCleanup(() => {});
};
installOrphans();

export const Widget: Component = () => {
  createEffect(() => 2, () => {});
  onCleanup(() => {});
  return null;
};

export const useOrphan = () => {
  createEffect(() => 3, () => {});
  onCleanup(() => {});
};
