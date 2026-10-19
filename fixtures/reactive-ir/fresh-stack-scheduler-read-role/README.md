# fresh-stack-scheduler-read-role

A read in a callback handed directly to a reviewed fresh-stack scheduler (`FRESH_STACK_SCHEDULERS`, matched by compiler-selected declaration) does not run in the scheduling body, so it is no strict-read violation (`ExecutionRole::DeferredCallback`); body reads and local lookalikes stay violations; `addEventListener`, `bind`, `PromiseLike.then`, `setTimeout(wrap(...))` run on an invoker's stack and stay uncertifiable (`host-callback-read-timing`). Stub: `fresh-stack-scheduler-owner`'s (published rc.3).
