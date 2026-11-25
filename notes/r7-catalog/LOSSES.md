# R7 losses against the accepted costs (main 7bc723e35)

Source of the costs: `docs/decisions.md` on main (ADRs 1189, 1194, 1196, 1198); `scripts/fuzz/proved/changes.json` has no rows for them.

## 236 mx-recall STRICT_READ_UNTRACKED agree -> recall-gap (ids: vu236.json.gz)
- Pre-1166 main 01d5fdc5f (release, VM, focus.mjs, Chrome judge): all 236 recall-gap.
- 0751069bf (with 1166): all agree. 60a7cf1db (1189): recall-gap (R7 bisect). 7bc723e35 (after 1194): recall-gap.
- So each is one of 1166's 3,943 moved to agree, sent back by 1189, not won back by 1194: inside the 1,503
  (1194, Not built: a declarer reached through a function handed on as a value, a reference the list lacks, a chain past four).
- None outside.

## 20 lg-recall STRICT_READ_UNTRACKED clean -> uncertifiable (effect-cleanup-show/-for)
- Reasons on 7bc723e35 (release checker, each program):
  - 12 `store-tuple-{element,setter,handed}` x show/for x direct/alias: SC9012 `store-key-undecided`;
  - 8 `store-settled-carried`, `store-settled-named-carried` x show/for x direct/alias: SC1001 `function-run-unplaced` at `[f].forEach((g) => g())`.
- ADR 1198's account of 1196's remaining 50: 38 SC9012 dispatch withholding rows (`store-key-undecided` among the kinds), 8 SC1001 reads carried by `[f].forEach`, 4 SC1003.
- The 12 are within the 38 and the 8 are the 8: inside the 50. None outside.
