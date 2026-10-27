import { describe, it } from "vitest";
import { fireAndFlush } from "./helpers";

const node = document.createElement("input");

describe("card", () => {
  it("fires", () => {
    fireAndFlush(() => node.dispatchEvent(new Event("change")));
  });
});
