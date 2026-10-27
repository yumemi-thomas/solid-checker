import { describe, it } from "vitest";
import { fireAndFlush } from "./helpers";

const node = document.createElement("input");

// An async helper that suspends before it fires: the part after the await
// runs with no owner and no window whoever called it, and the part before runs
// on its caller's stack, a test's.
async function settleThenFire() {
  await Promise.resolve();
  fireAndFlush(() => node.dispatchEvent(new Event("settle")));
}

// A factory whose returned object holds the helper; the test destructures it.
function renderCard() {
  const open = () => fireAndFlush(() => node.dispatchEvent(new Event("open")));
  return { open };
}

describe("card", () => {
  it("settles", async () => {
    await settleThenFire();
  });

  it("opens", () => {
    const { open } = renderCard();
    open();
  });
});
