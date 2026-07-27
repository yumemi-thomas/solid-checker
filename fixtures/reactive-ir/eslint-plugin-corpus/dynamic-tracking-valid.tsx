import { dynamic } from "@solidjs/web";
import { createMemo } from "solid-js";

const tag = createMemo(() => "div");

export const DynamicTag = dynamic(() => tag());
