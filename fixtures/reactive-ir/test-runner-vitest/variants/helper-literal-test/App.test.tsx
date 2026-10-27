import { describe, it } from "vitest";
import { count, setCount } from "./Card";
import { fireAndFlush } from "./helpers";

const node = document.createElement("input");

describe("card", () => {
  it("fires", () => {
    // The literal runs inside the helper's call, on the runner's stack: the
    // dispatch leaves the veto, its write and read are clean.
    fireAndFlush(() => node.dispatchEvent(new Event("change")));
    fireAndFlush(() => setCount(count() + 1));
  });
});
