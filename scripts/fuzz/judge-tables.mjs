// The judge's tables as JSON (endstage-report.md 1d): a change to any of them is an edit to judge-frozen.json too.
import { besideCode, codeRules, decliningRules, handedCodes, ignoredRules, partlySilentRules, placelessCodes, thrownCodes, withholdingRules } from "./codes.mjs";

export const judgeTables = () => ({
  codeRules, withholdingRules, handedCodes, decliningRules, ignoredRules, besideCode, partlySilentRules, placelessCodes,
  thrownCodes: thrownCodes.map(([pattern, code]) => [String(pattern), code]),
});
