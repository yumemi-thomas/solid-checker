// kui kernel dialog.test.tsx:46: a test helper dispatches a click. Unexported,
// its callers are enumerable in an open program too, so it leaves the veto
// (ADRs 0438, 0640); an exported one withholds every handler's proofs.
import { describe, expect, it } from "vitest";
import { render } from "@solidjs/web";
import { Select } from "./select";

function clickOn(element: HTMLElement): void {
  element.dispatchEvent(new MouseEvent("click", { bubbles: true }));
}

describe("Select", () => {
  it("opens on click", () => {
    const host = document.createElement("div");
    const dispose = render(() => <Select options={["a", "b"]} />, host);
    clickOn(host.querySelector("button")!);
    expect(host.querySelectorAll("li").length).toBe(2);
    dispose();
  });
});
