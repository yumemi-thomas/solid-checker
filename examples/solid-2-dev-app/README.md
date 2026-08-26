# Solid 2 development app

This standalone Vite app demonstrates the complete local development path:
Solid 2 source, native `solid-checker` analysis, ephemeral handoff to Oxlint, and
framed diagnostics through the JavaScript adapter. No snapshot file is exposed
to the user.

## Run the app

```sh
cd examples/solid-2-dev-app
bun install
bun run dev
```

`src/App.tsx` is the clean, runnable application. Verify it with:

```sh
bun run lint:clean
```

## Paired rule examples

`src/cases` is an intentionally exhaustive teaching and checker corpus. Every
file covers one rule and keeps its `Bad...` and `Good...` components together,
so the unsafe pattern and its replacement can be compared without jumping
between directories.

| File | Cases covered |
| --- | --- |
| `component-props.tsx` | parameter, body, and aliased destructuring; direct reads, defaults, and prop forwarding |
| `untracked-reads.tsx` | component-body, aliased, and effect-apply reads; JSX, memo, and effect-compute reads |
| `reactive-writes.tsx` | component, memo, and effect-compute writes; event, settled, and untracked writes |
| `actions.tsx` | action calls in owned scopes; event-driven actions and writes inside actions |
| `effect-cleanup.tsx` | forbidden nested cleanup registration; cleanup returned by all three supported effect forms |
| `leaf-owners.tsx` | primitive creation and `flush()` in leaf owners; leaf-only work |
| `control-flow-accessors.tsx` | `Show`, all `For` keying modes, and `Repeat` callback argument shapes |
| `async-after-await.tsx` | reactive reads after one or several awaits; dependency capture before suspension |
| `loading-boundaries.tsx` | missing, fake, direct, wrapped, and errored async boundaries |
| `refresh-targets.tsx` | wrapper/read misuse versus original memo and signal targets |

There are 51 example components arranged into bad/good comparisons. Type-check
their source shapes with:

```sh
bunx --bun --no-install tsc --noEmit --skipLibCheck --lib ESNext,DOM
```

Run all semantic examples with:

```sh
bun run lint:examples
```

That command is expected to exit non-zero because every file deliberately
contains violations. It is also a living conformance corpus: good examples
that receive a diagnostic and bad examples that do not are useful evidence of
checker gaps, rather than reasons to hide either half of the pair.

`src/showcase/AsyncLoadingBoundary.tsx` also includes both a direct uncovered
async read and a custom wrapper that does not provide a real `<Loading>`
boundary. The showcase directory focuses on multi-file and proof-specific
demonstrations; `src/cases` focuses on small side-by-side rules.
Display these expected proof-backed failures with:

```sh
bun run lint:failing
```

Apply the safe fix for its simple parameter destructure with:

```sh
bun run lint:fix -- src/BadCard.tsx
```

The equivalent generic Bun form is `bun run lint -- --fix src/BadCard.tsx`.
`bun run lint --fix` does not forward `--fix`; Bun treats it as a Bun option.
Aliases, defaults, rest patterns, writes, and references outside
compiler-recorded JSX expression containers remain diagnostic-only.

The failing command exits non-zero and uses Oxlint's `default` formatter, so it
shows the source frame, highlighted destructuring pattern, canonical `SC1003`
identifier, proof evidence, and summary.

The app installs the private workspace `solid-checker` package for project-level
certification and runs Oxlint independently for syntax linting. There is no
ESLint compatibility adapter or shared snapshot between the two tools.
