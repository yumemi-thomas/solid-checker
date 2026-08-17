import { createMemo } from "solid-js";

export const helper = () => "/helper";

export const importedTracked = createMemo(() => "/imported");

const defaultTracked = createMemo(() => "/default");
export default defaultTracked;
