# ADR 0254: Exact receiver members and optional contract reads

- Status: accepted and implemented (2026-10-08).
- Owners:
  - receiver member binding in `bind_returned_member_effects`
    (`contracts.rs`) and `local_access.rs`;
  - `ContractReadContext::at_call` (`lib.rs`) and its projection;
  - the `notification` spec.
- Fixture: `fixtures/reactive-ir/package-member-receiver-consumer`.
- Relation: census-2 items C-members, C-read-count and A-notification.
  Drafted in `rust/target/research/members/`.

## Decision

1. **A direct member call on an exact receiver is bound.**
   `const panel = createPanel(); panel.stop()` instantiates `stop`'s stated
   graph, as `const { stop } = createPanel()` already did (ADR 0235). This
   needs:
   - an immutable single-name binding whose initializer is exactly the factory
     call;
   - a closed, agreed object return;
   - references matched through the binder (ADR 0250).

   Any escape of the receiver, a write or deletion, a computed member, or a
   cast receiver withholds the graphs, as an obligation. `(panel.stop)()` is
   the same call as `panel.stop()`, since the parentheses keep the receiver.
2. **A contract read keeps the caller's execution role. Its count and timing
   decide whether it is proven.** An optional (`min: 0`), guarded, later or
   untimed read is unproven: uncertifiable in an untracked component, nothing
   in JSX. The draft also made every read stated `untracked` untracked even
   inside JSX, which would have changed how every hand-stated contract reads
   and demoted proven findings. That part is not taken.
   `strictRead: cleared` (ADR 0247) stays the one explicit way to say a
   package cleared a read.
3. **`createNotification`'s `notification` member states its read.** The
   draft's `hostFree` block described only the server path, which is
   unsound: a run with no host may be in the browser (ADR 0230), and it made
   two none misuses silent. It is dropped; `createNotification` has no
   host-free claim, as before. The other use of `hostFree.call`
   (static-store, ADR 0253) was checked: it is exactly the previously
   reviewed graph.

## Consequences

- Primitives ledger, browser: 92 of 112 report correctly (was 91). No
  correct twin on any host has a violation.
- rc.13 sweep: no violation added or lost; uncertifiable 3507 to 3504.
- `partial-structural-return-consumer`'s `UnknownObjectMember` keeps its
  obligation, now located at the factory call rather than at `panel.stop`.
