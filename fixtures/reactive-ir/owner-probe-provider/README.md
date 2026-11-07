# An owner probe proves SC4002's run (ADR 0866 amendment)

`if (getOwner())` drops the owner questions of the guarded operation: it runs only
with an owner. A provider-free context read is the opposite claim: with an owner and
no provider, rc.21 throws CONTEXT_NOT_FOUND (Chrome, fuzz `flw-guard-early`). The probe
no longer drops that requirement.
