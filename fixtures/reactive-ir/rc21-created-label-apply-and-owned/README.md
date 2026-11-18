# A helper also handed to `runWithOwner` keeps the veto

ADR 1099's guard. `pick` is also passed to `runWithOwner(owner, pick)` from a second button, a reference that is not a call in an
apply: that delivery runs the handler under the component's owner (Chrome: `ContextNotFoundError`), the apply's delivery with none
(`NoOwnerError`). The handler stays open: SC4001 and SC4002 uncertifiable. Chrome: `runtime-oracle/created-label-apply-and-owned`.
