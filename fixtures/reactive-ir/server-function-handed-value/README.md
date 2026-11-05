# server-function-handed-value

ADR 0704. A server function read as a value (an alias, a shorthand object slot, a higher-order argument, `.call`, a module
directive file's export handed on) reaches calls SC7007's call loop never sees. Each such reference outside server code is
uncertifiable, blocker `server-function-value-handed`. Negatives: the direct call (the existing rule: `new Date()` is a
violation), a type query, a reference inside another server function and one in a module-directive file. Runtime not measured:
no host compiles `"use server"`.
