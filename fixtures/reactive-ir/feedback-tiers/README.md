# feedback-tiers

**Claim (ADR 0202).** In a closed program (a private `package.json`, ADR 0193), an exported helper's contract-generation obligation is not reported: no consumer reads this project's contract. In both, an unresolved member dispatch is reported where its call runs in the component body (line 18) and not where it runs in a click handler (line 20): no read rule reports what runs there.

`feedback-tiers` and `feedback-tiers-open` share `App.tsx`; only `package.json` differs. `diagnostics_process::default_output_groups_coverage_gaps_after_findings_to_review` pins the default output's tiers on both. The stubs are copied from `forwarded-event-prop`.
