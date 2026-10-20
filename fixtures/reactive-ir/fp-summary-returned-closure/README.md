# fp-summary-returned-closure

**Claim.** A closure a hook builds into its result (object data property/getter, or element of a returned tuple) is not run by the hook's call, so its reads are neither the hook's summary reads nor on its call edges (`runs_in_retained_value_literal`) and a component merely calling the hook is not charged; call-argument literals keep callee-timing attribution; later calls project the closure's reads only with exact transport and body facts. Stub copies `createSignal`/`createMemo` verbatim from rc.9.
