# Variant `eval`

The eval cases of `../../README.md` (`Eval.tsx`), in a project of their own. Their claims are unchanged. ADR 0803 (item 3): the direct `eval` may patch array iteration, so every primitive's tuple slot here is withdrawn too, and each use of one is uncertifiable `tuple-iteration-written` beside the case's own row.
