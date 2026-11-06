# exported-const-copy

**Claim (ADR 0821).** `export const copied = count`, a `const` copy of a `const` source, holds that source wherever it is imported: an untracked read and a store write through the copy are proved, a read in a memo stays clean. A plain value's copy states nothing.
