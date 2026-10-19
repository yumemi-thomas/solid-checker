# feedback-tiers-open

**Claim (ADR 0202).** In an open program (no `package.json` above the fixture), the same helper's
contract-generation obligation is still reported (`SC9005`, line 8). In both this and `feedback-tiers` (same
`App.tsx`; only `package.json` differs), an unresolved member dispatch is reported where its call runs in the
component body (line 18) and not in a click handler (line 20).
`diagnostics_process::default_output_groups_coverage_gaps_after_findings_to_review` pins the default output's
tiers on both. The stubs are copied from `forwarded-event-prop`.
