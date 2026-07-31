import { createMemo, createSignal } from "solid-js";
import * as solid from "solid-js";

const [count, setCount] = createSignal(0);
const setterAlias = setCount;
createMemo(() => setterAlias(count() + 1));

const [, setNamespaceValue] = solid.createSignal(0);
solid.createMemo(() => setNamespaceValue(1));
