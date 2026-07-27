import {
  action,
  createMemo,
  createProjection,
  createSignal,
} from "solid-js";
import * as solid from "solid-js";

const [count, setCount] = createSignal(0);
const setterAlias = setCount;
createMemo(() => setterAlias(count() + 1));

const save = action(function* () {});
const saveAlias = save;
createMemo(() => saveAlias());

const [, setNamespaceValue] = solid.createSignal(0);
solid.createMemo(() => setNamespaceValue(1));

const [, setProjectionValue] = createSignal(0);
createProjection((draft: { value: number }) => {
  setProjectionValue(1);
  draft.value = count();
}, { value: 0 });
