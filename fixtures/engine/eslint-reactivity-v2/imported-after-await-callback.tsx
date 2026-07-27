import { createMemo } from "solid-js";
import { compute } from "./imported-after-await-definition";
export const value = createMemo(compute);
