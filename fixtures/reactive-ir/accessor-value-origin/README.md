# accessor-value-origin

**Claim (ADR 0211, phase 2).** An accessor call's value is proven built-in when every value the accessor can hold is (`accessor_origin`): a `createSignal` initial value and every setter write, a `createMemo` compute's returns, an exact project function's returns (a returned parameter proves nothing), both operands of `??`/`||`/`&&`/`?:`; then the `SC9012` dispatch obligation on `filter` leaves, while escaped setters, written stores, writable memos, store memos and returned parameters keep it.
