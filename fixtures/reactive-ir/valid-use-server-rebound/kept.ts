"use server";

// Clean: a member write does not change the registered function.
export let h = async () => {};
(h as any).label = "h";

// Violation: a literal export.
export const k = 1;
