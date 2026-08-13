# execution-map-incomplete

`SC9004` · **error** · uncertifiable

The Solid compiler did not classify a JSX expression as tracked, untracked, or a
callback.

## What it does

Flags JSX expression positions for which the compiler facts carry no execution
role. Every reactive-read rule depends on knowing whether a position tracks;
without a classification, reads inside the expression can be neither certified nor
proven wrong.

## Why is this analysis-limiting?

solid-checker's read analysis is anchored on the compiler's execution map: each JSX
region is tracked (subscribes), untracked (runs once), or a callback (runs later
under its own rules). A gap in that map usually means the expression's shape falls
outside what the JSX compiler recognizes — or that the compiler facts on disk are
stale relative to the source.

## Coverage boundary

The controlled compiler producer currently classifies every supported JSX site
or rejects malformed facts before rule analysis starts. Therefore an ordinary
fresh project analyzed with the bundled producer should not reach `SC9004`.

The rule remains fail-closed for custom, stale, or future fact producers. Its
diagnostic wording is covered by the synthetic catalog-prose program, while the
execution-map validation tests inject missing and conflicting classifications.

## How to fix

Two things to try, in order:

1. **Simplify the expression.** Hoist complex logic into a `createMemo` and
   interpolate the accessor — simple interpolations always classify:

   ```tsx
   // Instead of an exotic inline expression:
   const label = createMemo(() => buildLabel(user(), locale()));
   return <span>{label()}</span>;
   ```

2. **Refresh compiler facts.** If the flagged expression is plain JSX, the facts
   may be stale — re-run the analysis cold with the bundled producer. If the
   finding persists, report the JSX pattern and compiler options as a
   solid-checker issue; it indicates a producer or compatibility defect.

## Related

- [strict-read-untracked](strict-read-untracked.md) — what the execution map feeds
