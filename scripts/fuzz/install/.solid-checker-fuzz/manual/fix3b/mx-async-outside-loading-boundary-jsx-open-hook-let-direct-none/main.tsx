import { render } from "@solidjs/web";
import * as mod from "./App";
import { C1 } from "./App";

// Each exported component is rendered as a component, so Solid creates it the
// way an application does, with the component's strict-read window open, and
// in its own root, so one component's failure does not hide the others. The
// exports the case's "mount" names are rendered by their own top-level render
// call, in the namespace's (sorted) order, so a reader of this entry sees where
// they mount (ADR 0501); the loop renders every other export as before.
const root = document.getElementById("root")!;
const host = (name: string) => {
  const section = document.createElement("section");
  section.dataset.export = name;
  root.append(section);
  return section;
};
try {
  render(() => <C1 />, host("C1"));
} catch (error) {
  console.error("render failed", "C1", error);
  setTimeout(() => { throw error; });
}
const mounted = new Set<string>(["C1"]);
for (const [name, value] of Object.entries(mod as Record<string, unknown>)) {
  if (typeof value !== "function" || !(name === "default" || /^[A-Z]/.test(name)) || mounted.has(name)) continue;
  const Component = value as (props: {}) => any;
  try {
    render(() => <Component />, host(name));
  } catch (error) {
    console.error("render failed", name, error);
    // Rethrown from a timer, so the uncaught error reaches the feedback run's
    // source mapping as a runtime exception (ADR 0352); the others still render.
    setTimeout(() => { throw error; });
  }
}

// Drive every handler: bubbling events on every element, two passes apart.
const events: Array<[string, (type: string) => Event]> = [
  ...["click", "dblclick", "mousedown", "mouseup", "mouseover", "mouseout", "pointerdown", "pointerup", "contextmenu"]
    .map((type): [string, (type: string) => Event] => [type, (t) => new MouseEvent(t, { bubbles: true, cancelable: true })]),
  ...["keydown", "keyup"].map((type): [string, (type: string) => Event] => [type, (t) => new KeyboardEvent(t, { bubbles: true, key: "a" })]),
  ...["focus", "blur", "focusin", "focusout"].map((type): [string, (type: string) => Event] => [type, (t) => new FocusEvent(t, { bubbles: true })]),
  ...["input", "change", "submit", "scroll", "wheel"].map((type): [string, (type: string) => Event] => [type, (t) => new Event(t, { bubbles: true, cancelable: true })]),
];
const pause = (ms: number) => new Promise((done) => setTimeout(done, ms));
async function exercise() {
  await pause(200);
  for (let pass = 0; pass < 2; pass += 1) {
    for (const element of Array.from(root.querySelectorAll("*"))) {
      for (const [type, make] of events) {
        try {
          element.dispatchEvent(make(type));
        } catch (error) {
          console.error("handler threw", type, error);
        }
      }
    }
    await pause(200);
  }
  // Give effects, timers and settled callbacks a moment, then mark readiness.
  await pause(400);
  document.getElementById("ready")!.textContent = "settled";
}
void exercise();
