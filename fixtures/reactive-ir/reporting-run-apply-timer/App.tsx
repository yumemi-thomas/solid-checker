import { createEffect, createSignal } from "solid-js";

// ADR 1166 r3 (corpus kui GroupRoute.tsx:80): a `const` helper declared in an effect's apply reads a prop; the apply calls it once
// right after the declarator, and the helper re-arms itself from a timer. Does the apply's run warn STRICT_READ_UNTRACKED?
function Group(props: { id: string }) {
  const [attempt] = createSignal(0);
  createEffect(
    () => [props.id, attempt()] as const,
    () => {
      let cancelled = false;
      let timer: ReturnType<typeof setTimeout> | undefined;
      const refresh = (background: boolean): void => {
        void Promise.resolve(props.id).then(() => {
          if (cancelled || background) return;
          timer = setTimeout(() => refresh(true), 10);
        });
      };
      refresh(false);
      return () => {
        cancelled = true;
        if (timer !== undefined) clearTimeout(timer);
      };
    },
  );
  return <p>group</p>;
}

export function App() {
  const [id] = createSignal("a");
  return <Group id={id()} />;
}
