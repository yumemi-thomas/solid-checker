"use server";
// A module-level directive makes the whole module server code: a stray
// function-level string, and a directive in an object method, are harmless
// (silent). A template literal is still no directive — finding.
export async function getUser(id: string) {
  const table = "users";
  "use server";
  return [table, id];
}

const api = {
  async getUser() {
    "use server";
    return 1;
  },
};
export async function useApi() {
  return api;
}

`use server`;
