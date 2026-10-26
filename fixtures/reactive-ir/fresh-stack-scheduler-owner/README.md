# fresh-stack-scheduler-owner

A callback handed directly to a reviewed fresh-stack scheduler (`queueMicrotask`, `setTimeout`, `setInterval`, `requestAnimationFrame`, `requestIdleCallback`, `Promise.then/catch/finally`, the observer constructors) runs with no owner wherever scheduled, so `onCleanup`, an effect or an `onSettled` returning a cleanup there is a proven `SC4001`; `PromiseLike.then`, `addEventListener` and lookalikes are none and `setTimeout(wrap(...))` a known false negative. Stub: `detached-callback-in-root`'s (published rc.3).

The components are exported: a function nothing renders is no proven component body (ADR 0403).
