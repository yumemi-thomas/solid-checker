# `from`

`from(subscribe)` calls nothing; the iterator a computation asks for does, in that
pass, with an owner and an observer. Never at the call, and never without a
consumer. Measured on the rc.21 dev client build (ADR 0622).
