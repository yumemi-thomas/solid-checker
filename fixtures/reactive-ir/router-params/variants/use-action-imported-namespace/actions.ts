import { action } from "@solidjs/router";

declare function store(value: unknown): void;

export const save = action(async (form: FormData) => {
  store(form);
});
