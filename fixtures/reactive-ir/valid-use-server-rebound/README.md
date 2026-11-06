# SC7006: registration is the initializer at the declaration

ADR 0702: rc.21 registers each export's initializer at its declaration, so a later reassignment never reaches registration. Rebound function clean (`rebound-function.ts`), and a member write on an export changes nothing (`kept.ts`); rebound literal and literal export violations (`rebound-literal.ts`, `kept.ts`).
