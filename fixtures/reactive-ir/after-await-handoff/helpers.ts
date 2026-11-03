// A wrapper in another module: ADR 0565's proof is same-file only.
export function run(g: () => void): void {
  g();
}
