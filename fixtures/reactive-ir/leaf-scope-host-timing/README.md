# Audited host callback timing (HT)

Candidate fixture; not executed. These are leaf-rule expectations. Host types
come from the default library, never local structural replacements. The
positive NodeList examples populate their collection before iterating.

| Case | Role | Expected leaf finding |
| --- | --- | --- |
| PromiseExecutorForbidden | positive | SC3001 violation: executor invokes register synchronously |
| NodeListForbidden | positive | SC3001 violation: populated childNodes invokes register synchronously |
| PromiseExecutorClean | negative | none: empty executor body |
| NodeListClean | negative | none: synchronous callback only sets an attribute |
| MediaActionClean | negative | none: registration; handler delivery is a fresh task |
| MediaQueryClean | negative | none at registration; listener is deferred, not guaranteed fresh |
| GlobalListenerClean | negative | none at registration for both global spellings |
| ExistingObserverRows | existing behavior | none: five constructors already have timing entries |
| WrappedNodeListCallback | wrapper | SC9012 uncertifiable: the wrapped identifier is not followed to its function (a missed proof, not a false positive) |
| SameNamedProjectMethod | member boundary | SC9012 uncertifiable: matching method spelling is not a host fact |
| ShadowedGlobal | shadowing | SC9012: caller-supplied registration function |
| PromiseResolveStillOpen | unresolved | SC9012: executor resolve parameter is not specialized |
| MediaQuerySynchronousDispatch | synchronous dispatch | none: the listener is deferred; synchronous dispatch is not modeled (ADR 0210) |
| GlobalSynchronousDispatch | synchronous dispatch | none: the same for a global listener |
| ListenerObjectStillOpen | object listener | none: a `handleEvent` object is a deferred listener too |

Exact synchronous dispatch remains unmodeled (ADR 0210). The new registration
slots therefore require a proven clean callback body even though registration
itself does not invoke it. No new row claims these listeners run fresh.

solid-js.d.ts is copied byte-for-byte from leaf-scope-host-callbacks; no
signature was widened. Real published-type tsc validation, producer declaration
replay, coverage, and snapshots are deferred by the task constraint. Promotion
requires the per-fixture .gitignore exceptions and reviewed snapshots.
