// Project code reached through the "@/*" tsconfig path alias. The engine
// walks this body directly, so no package contract is needed to certify the
// accessor App.tsx passes in.
export function consume(source: () => number): number {
  return source();
}
