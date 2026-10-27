import { describe, it } from "vitest";
import { renderCard } from "./render";

describe("card", () => {
  it("opens", () => {
    const { open } = renderCard();
    open();
  });
});
