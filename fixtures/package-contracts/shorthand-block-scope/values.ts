import { createMemo } from "solid-js";

export const helper = () => "/helper";

export const importedTracked = createMemo(() => "/imported");
