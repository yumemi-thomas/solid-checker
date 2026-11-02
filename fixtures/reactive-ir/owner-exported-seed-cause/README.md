# owner-exported-seed-cause

An owner operation in an exported helper's own body (open program) is an
obligation whose cause is the helper's unseen callers: its open-world seed is
its owner edge, so ADR 0534's unplaced-callback clause does not replace the
exported-caller clause. A callback the helper hands to a scheduler, or one
`forEach` runs, has no owner edge and keeps ADR 0534's clause. All three stay
uncertifiable; the snapshot keeps the wording, which is the claim. rc.17 types
reduced in `solid-js.d.ts`.
