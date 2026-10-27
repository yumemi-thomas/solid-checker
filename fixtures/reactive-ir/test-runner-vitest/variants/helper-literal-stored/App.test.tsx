import { describe, it } from "vitest";
import { later } from "./helpers";

const node = document.createElement("input");

describe("card", () => {
  it("queues", () => {
    later(() => node.dispatchEvent(new Event("change")));
  });
});
