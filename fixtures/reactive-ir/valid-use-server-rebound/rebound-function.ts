"use server";

// Clean: the compiler registers the initializer where it is declared
// (`registerServerReference("f-…", async () => {})`), so the later
// reassignment never reaches registration.
export let f = async () => {};
f = 1 as any;

// Clean, the specifier form: same, the declarator's initializer is registered.
let g = async () => {};
g = 1 as any;
export { g };
