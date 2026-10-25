import { describe, it } from "vitest";
import { choose } from "./index";

const node = document.createElement("input");

describe("card", () => {
  it("chooses", () => {
    choose(node);
  });
});
