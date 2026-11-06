# Variant `eval`

The eval case of `../../README.md` (`DirectEval`, `Eval.ts`), in a project of its own. Its claim is unchanged: uncertifiable. ADR 0803 (item 3): the direct `eval` may patch array iteration, so the store's tuple slot is withdrawn too, and each use of it is uncertifiable `tuple-iteration-written` beside the case's own row.
