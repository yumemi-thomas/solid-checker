# dead-code-module-helpers

**Claim (ADR 0578).** ADR 0458's site-reach query took every module-scope function to run ("entered by modules and hosts this does not follow"). In a module, a function bound to a name the module does not export is entered only by what names it, as one written in a function is: when every call and reference of the name is unreached (and no module calls direct `eval`), it never runs, and a violation in it is uncertifiable. A script's top level is global and keeps the old answer (`flow_binding_exported`).

Uncertifiable (`missing-context-provider` unless noted): `readInIf` (`if (false)`), `readAfterReturn`, `readAfterFalse` (`false &&`), `readArrow` (a `const` arrow), `Child1` (a component nothing renders or names), `readInLazy` (only an unread lazy memo calls it, ADR 0576), `writeInIf` (`no-write-in-pure-computation`, called only in a memo's dead branch).

Violations: `readLive`, `readLiveArrow`, `Child2`, `writeLive`: the same helpers reached.

Premises: the closed program is the `private` manifest. Runtime: fuzzer cells `lg-recall` CONTEXT_NOT_FOUND `dead-if`, `dead-after-return`, `dead-and-false`, `dead-comp-never` (`wrapper` binding; 10 of 10 sampled `unexecuted`, Chrome-confirmed: the site's counter stays 0).
