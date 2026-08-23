# Package unknown-returns consumer

`{ "status": "unknown" }` is a claim in five independent domains, and the
consumer treats them independently: `callbacks` is demanded only where a call
passes a callable (pinned by `package-unknown-callback-consumer`), while
`reactiveReads`, `returns`, `ownerRequirements`, and `asyncBehavior` are
opened where the claim enters the project. This fixture pins that second,
non-callback half.

- `openSource` carries `returns: { "status": "unknown" }`. Its import binding
  must carry one `package-contract-incomplete` uncertifiable finding
  (`SC9005`), whose analysis context names the exact domain
  (`unknown-contract-claims:returns`) rather than the whole summary. The
  contract states its other four domains, and they must stay usable.
- `openPlain` is the negative: the same package, the same shape of use, a
  summary with nothing unknown. An omitted `returns` field is a reviewed
  claim that the return carries no reactivity, so this import binding and both
  of its reads stay clean.

`cli_reports_the_exact_unknown_claim_domain` in
rust/crates/solid-facts-backend/tests/contracts_process.rs pins the domain
string, which the findings snapshot deliberately does not record.

The declaration is exact for this fixture package; the finding depends on the
runtime contract, not on trusting the declaration as runtime evidence.
