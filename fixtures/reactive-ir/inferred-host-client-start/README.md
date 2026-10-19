# Inferred host: client-start

Copy of `inferred-host-spa` with a generated client entry that supplies the exact default App body and module root; default-body roots require module completion (a process regression makes initialization throw before the wrapper calls the default export, whose timer callback then keeps baseline). No published signature is loosened.
