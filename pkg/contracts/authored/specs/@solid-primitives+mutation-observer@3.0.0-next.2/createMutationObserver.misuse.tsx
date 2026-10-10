import { createMutationObserver } from "@solid-primitives/mutation-observer";
createMutationObserver(document.body, { childList: true }, () => {});
