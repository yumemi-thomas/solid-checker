# reactivity-group-key

Claim (ADR 0428): the `reactivity` key in `.solid-checker/rule-options.json` sets `strict-read-untracked` (SC1001), `reactive-read-after-await` (SC1002), `uncalled-accessor` (SC1005) and `reactive-handler-frozen` (SC1007) together, and a member the same file names keeps its own setting. `App.tsx` holds one proven defect of each member plus an SC1003 (`no-destructure`) defect, which the key never reaches. This project, with no options, reports all five.

- `group-off`: `reactivity` disabled; only SC1003 remains.
- `member-wins-group-listed-first` and `member-wins-group-listed-last`: the group is disabled and two members are enabled (`reactive-read-after-await` sorts before `reactivity`, `uncalled-accessor` after it); the two files differ only in key order and report the same SC1002, SC1005 and SC1003.
- `group-on-member-off`: the group is enabled and `uncalled-accessor` disabled; SC1005 alone disappears.
