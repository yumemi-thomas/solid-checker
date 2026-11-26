# ADR 1219 sweep: legal violations at a site behind a false run-time test

Run: `node scripts/fuzz/focus.mjs --codes <all 30> --grids lg-recall -- --ids <every flw-branch-false and flw-branch-false-helper id>` (50,062 programs), main 60de70ac7, release, VM, Chrome judge.
`legal-v-ids-by-rule.json.gz`: per violating rule, every program id with that legal violation. Not for merge.
