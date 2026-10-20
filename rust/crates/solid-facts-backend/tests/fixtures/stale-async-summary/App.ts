import { createMemo } from "solid-js";
import { load } from "reactive-package";

export const value = createMemo(load, { sync: true });
