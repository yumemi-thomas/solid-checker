# One entrypoint's open obligation stays inside that entrypoint

Three published entrypoints share one package:

- `.` re-exports `readRoot` from `implementation.ts` -- fully described;
- `./broken` re-exports `hiddenScheduler` from the *same* file, and that
  function forwards its callback to an ambient `declare function` whose timing
  the artifact does not contain, so its callback domain stays open;
- `./feature` reaches a different file entirely.

`unrelated.ts` contains the same unresolvable forwarding but is published by no
entrypoint, so it must not appear anywhere.

The claim is per-entrypoint attribution: the open claims recorded in the review
plan belong to `./broken`, and `.` and `./feature` keep their complete
descriptions. Analyzing the package as one program -- or letting one file's
unresolved callee open every entrypoint that shares that file -- would turn a
single unavailable declaration into a package-wide loss of precision.
