import { action } from "@solidjs/router";

declare function store(value: unknown): void;

// The router's own action wrapper, exported. Every file that holds it is
// analyzed, and each use of it is a safe one (see `App.tsx`).
export const save = action(async (form: FormData) => {
  store(form);
});
