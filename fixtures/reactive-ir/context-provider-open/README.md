# context-provider-open

**Premise (ADR 0352, amended by ADR 0517).** Without a manifest the program is open: unseen code may provide the context, so `Open` is no proven `ContextNotFoundError`. The census still runs, and a read of a context it finds no provider for is an uncertifiable obligation (SC4002), not a certified clean: rc.17 throws there when no provider exists.
