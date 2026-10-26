// Violation. Corpus: app-game piecs-examles/performance/performance.tsx:32, jandibat-web
// apps/web/src/pages/AuthPage.tsx:347: `.map` on a reactive value in a JSX child builds the rows
// without keyed reconciliation; For does.
import { createSignal } from "solid-js";

export function Sessions() {
  const [sessions] = createSignal([{ id: "a" }, { id: "b" }]);
  return (
    <ul>
      {sessions().map((session) => (
        <li>{session.id}</li>
      ))}
    </ul>
  );
}
