# SC2003: a write through a shallow store

`createStore(value, { shallow: true })` serves every value raw (rules-mining
`core-store` R38, R39). Measured on rc.21: `state.list[0].a = 2` lands on the raw
object and warns nothing, and `state.list = []` is dropped with
`STORE_WRITE_OUTSIDE_SETTER`. So a write past the root of a store proven shallow
is silent, and the root write stays the rule's (ADR 0626).
