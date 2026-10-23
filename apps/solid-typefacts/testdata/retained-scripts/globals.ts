// No imports and no exports, so every file in this project is script-kind and
// shares one global scope. Identifier references therefore resolve to the
// declaration itself rather than to a per-file import alias, which is what lets
// one symbol be demanded from several files at once.

function scale(value: number): number {
  return value * 2;
}

const factor = 3;

async function loadValue(): Promise<number> {
  const base = await Promise.resolve(1);
  return scale(base);
}
