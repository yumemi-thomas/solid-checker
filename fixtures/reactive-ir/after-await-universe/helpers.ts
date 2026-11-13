// A project module the program reaches through a namespace import: a call `helpers.format(1)` after an await resolves to this declaration exactly.
export function format(n: number) {
  return JSON.stringify({ n });
}
