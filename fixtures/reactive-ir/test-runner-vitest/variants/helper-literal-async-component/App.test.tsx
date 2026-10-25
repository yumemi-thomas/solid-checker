import { describe, it } from "vitest";
import { settleThenFire } from "./settle";

describe("card", () => {
  it("settles", async () => {
    await settleThenFire();
  });
});
