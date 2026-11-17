import { installAll } from "fz-gany";
import { readThis } from "fz-gthis";
import { readWindow } from "fz-gwindow";
import { readBare } from "fz-gbare";

// `fz-gany` writes globals under any name and its closure is undecided (a computed `import`): it may write any of the three, so none is decided.
void installAll({ next: "./x.js" });
// Each package looks up a global no pinned library declares: `this.fzHelper` (a script's top-level `this`), `window.fzPolicy`, a bare `fzEnv`. Nothing in this
// program writes any of them (no package, no project file, no script of `index.html`), so each is the host's or absent, and the call runs no action.
export function ReadThis() {
  return <p>{readThis()}</p>;
}
export function ReadWindow() {
  return <p>{readWindow()}</p>;
}
export function ReadBare() {
  return <p>{readBare()}</p>;
}
