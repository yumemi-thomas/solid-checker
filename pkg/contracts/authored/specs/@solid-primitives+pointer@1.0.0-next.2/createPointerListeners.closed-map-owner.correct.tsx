import { createPointerListeners } from "@solid-primitives/pointer";

// Legal with the real Partial<OnEventRecord> typings; no handler survives.
createPointerListeners({ target: document.body, onDown: event => console.log(event.x), ondown: undefined });
