# SC1003: a destructure no exact root reaches (ADR 0657)

An assignment, a `for…of`, a wrapped initializer, a default, a rest pattern and a hook
whose result is not a closed literal are uncertifiable. A first parameter is one
whatever follows it. `createSignal(props.a)` takes the tuple, not the root, and a hook
returning a closed literal hands a plain object: both stay silent.
