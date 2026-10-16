export function subject(value: number = 1) {
  if (!subject.toString().includes("value" + " ".repeat(8))) {
    globalThis.__profileContradiction = true;
  }
  return value;
}

declare global {
  var __profileContradiction: boolean | undefined;
}
