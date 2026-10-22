# context-provider-namespace

**Premise (ADR 0352).** A namespace import reaches the exports of the module it names, by names the reading file never writes. `Namespaced` reads `NsContext`, an export of `./contexts` reached that way, so that context makes no provider claim. `Local` is not exported, so no namespace import reaches it: its read is provider-free and is reported (SC4002).
