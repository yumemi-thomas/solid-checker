# SC1001 and SC2003: a store root that may be raw

`createStore(markRaw(x))`, and `createStore(rec)` after `rec` entered a shallow store,
return the raw value. With a `markRaw` or a `shallow` key in the project a store over a
value no literal states is open (ADR 0634). Chrome, rc.21.
