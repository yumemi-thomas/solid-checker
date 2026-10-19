# Invocations in different non-reporting roles (ADR 0323)

A callback invoked in several roles that all keep reads out of every reporting role (tracked, deferred, event; e.g. a query hook's `key` in a lazy memo and `untrack(options.key)`) takes the no-claim role (`function_role` merge); the walk also follows function values into primitive callback slots that keep reads out of the window (never an effect's apply). Body or effect-apply invocations among them stay uncertifiable. Published rc.13 declarations from `standard-runtime-configuration`.
