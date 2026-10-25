// A directive inside an already-extracted server function is ignored by the
// pass (the whole body runs on the server) — silent, in a nested object method
// and in the function's own blocks (upstream reports the latter; ownership case
// `valid-use-server/stray-in-extracted-function/001`).
export async function outer() {
  "use server";
  const helpers = {
    inner() {
      "use server";
      return 1;
    },
  };
  const table = "users";
  if (table) {
    "use server";
  }
  return helpers.inner();
}
