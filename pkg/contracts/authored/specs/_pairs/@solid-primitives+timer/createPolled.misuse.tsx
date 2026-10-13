import { createPolled } from "@solid-primitives/timer";

createPolled(() => Date.now(), 1000);
