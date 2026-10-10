import { createPointerListeners } from "@solid-primitives/pointer";

createPointerListeners({ target: document.body, onDown: event => console.log(event.x) });
