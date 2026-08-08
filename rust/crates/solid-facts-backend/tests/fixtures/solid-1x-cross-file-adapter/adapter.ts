// File A of the cross-file pair. Solid 1.x `mapArray` allocates and returns a
// function; neither the list nor the mapper runs until that returned function
// runs. Whether the read of `scale` below is a live reactive read or dormant
// code therefore depends entirely on `consumer.ts`, which is the only place in
// the project that invokes the result.
import { createSignal, mapArray } from "solid-js";

const [scale] = createSignal(2);

export const scaled = mapArray(
  () => [1, 2, 3],
  (item) => item * scale(),
);
