// kui kernel dialog.test.tsx:46: a test helper dispatches a click. In a
// closed program the helper is test-only and leaves the veto (ADR 0438); in
// this open one its callers are not enumerable, so every handler's owner and
// read proofs in the package are withheld (ADR 0356).
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
