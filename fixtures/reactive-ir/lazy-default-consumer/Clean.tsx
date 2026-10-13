import { lazy } from "solid-js";
import { createLoop } from "dep-loop";

const frame = () => {};

// Clean: `start()` runs only from the button's event handler.
const EventLoop = lazy(async () => {
  function EventCube() {
    const [running, start] = createLoop(frame);
    return <button onClick={() => start()}>{String(running())}</button>;
  }
  return { default: EventCube };
});

export function CleanPage() {
  return (
    <main>
      <EventLoop />
    </main>
  );
}
