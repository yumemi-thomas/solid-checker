import * as solid from "solid-js";
import {
  createMemo,
  createRoot,
  createTrackedEffect,
  mapArray,
  onCleanup,
} from "solid-js";

createTrackedEffect(() => onCleanup(() => {}));
solid.createTrackedEffect(() => solid.flush());
createTrackedEffect(() => createMemo(() => 1));
createTrackedEffect(() => createRoot(() => 1));
createTrackedEffect(() => mapArray(() => [1], item => item));
