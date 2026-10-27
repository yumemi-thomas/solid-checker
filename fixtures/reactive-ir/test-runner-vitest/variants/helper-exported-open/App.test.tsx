import { describe, it } from "vitest";
import { choose } from "./helpers";

const node = document.createElement("input");

describe("card", () => {
  it("chooses", () => {
    choose(node);
  });
});
