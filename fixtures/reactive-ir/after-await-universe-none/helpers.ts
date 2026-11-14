// A project module the program reaches through a namespace import: a call `helpers.triple(2)` after an await resolves to this declaration exactly, and its closure reads nothing.
export function triple(n: number) {
  return n * 3;
}
