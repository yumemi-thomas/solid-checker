// hoxxes-briefing BriefingPage.tsx:106 (createWallClock, :152): a getter reads an
// accessor a project factory returns, whose only evidence is its declared
// `Accessor<Date>` type.
import { createSignal, type Accessor } from "solid-js";

function createWallClock(): Accessor<Date> {
  const [now] = createSignal(new Date());
  return now;
}

export function Board() {
  const now = createWallClock();
  const view = {
    get now() {
      return now();
    },
  };
  return <p>{view.now.toISOString()}</p>;
}
