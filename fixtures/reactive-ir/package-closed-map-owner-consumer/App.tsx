import { createRoot, onSettled } from "solid-js";
import { startClosed } from "reactive-package";
import * as Package from "reactive-package";

// Proven violation, including namespace symbol resolution and TS wrappers.
startClosed({ target: document.body, onDown: () => {} });
Package.startClosed(({ onDown() {}, target: document.body }) satisfies Parameters<typeof startClosed>[0]);

// Correct owned twin: clean.
createRoot(dispose => {
  startClosed({ target: document.body, onDown: () => {} });
  return dispose;
});

// Guard false; the original optional row still yields uncertifiable, never violation.
startClosed({ target: document.body, onDown: () => {}, ondown: undefined });
startClosed({ target: document.body });
startClosed({ onDown: () => {} });

declare const config: Parameters<typeof startClosed>[0];
declare const fn: () => void;
declare const key: "onDown";
declare const args: [Parameters<typeof startClosed>[0]];
// Guard unknown; every shape remains uncertifiable.
startClosed(config);
startClosed({ target: document.body, onDown: fn });
startClosed({ target: document.body, onDown: () => {}, ...config });
startClosed({ target: document.body, get onDown() { return fn; } });
startClosed({ target: document.body, [key]: () => {} });
startClosed({ target: document.body, onDown: () => {}, __proto__: null });
startClosed(new Proxy({ target: document.body, onDown: () => {} }, {}));
startClosed(...args);

// A local homonym supplies no external contract identity.
function Shadowed() {
  const startClosed = (_config: unknown) => {};
  startClosed({ target: document.body, onDown: () => {} });
}
Shadowed();

export function Leaf() {
  onSettled(() => {
    startClosed({ target: document.body, onDown: () => {} }); // violation
    startClosed({ target: document.body, onDown: () => {}, ondown: undefined }); // uncertifiable
    startClosed(config); // uncertifiable
  });
  return <div />;
}
