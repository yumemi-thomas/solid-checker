// Imports nothing the edit script touches, so this file stays outside every
// affected set and its contributions must be retained.

export function scale(value: number): number {
  return value * 3;
}

export const scaled = scale(4);

export async function deepen(): Promise<number> {
  const base = await Promise.resolve(1);
  return scale(base);
}
