# absent-guard-prop

**Claim (ADR 0696).** Code that runs only behind a prop of the component is proved silent when every caller is in view and none passes that prop. `Absent` reports nothing. A caller that passes it (`Passed`), a spread at the call site, an exported component in an open program and a default on the guard keep what they had. A branch the checker cannot evaluate is no guard: `Unknown` keeps its violation.
