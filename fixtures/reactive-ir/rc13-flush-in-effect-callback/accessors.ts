// Accessors written outside App.ts: reading or writing one that cannot return
// is not a normal completion, so a `flush()` after it is not reached.
export const importedBox = {
  get value(): number {
    throw new Error("imported getter");
  },
};

export class ImportedClass {
  get value(): number {
    throw new Error("imported class getter");
  }
}

export const importedReturning = {
  get steady(): number {
    return 1;
  },
};
