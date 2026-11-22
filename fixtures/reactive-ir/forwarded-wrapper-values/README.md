# forwarded-wrapper-values

**Claim (ADR 1182).** The function a wrapper's name holds at a call is proved, where it is not the function first written for the name,
only when every value it can hold there is known:
- a written `let`: every write is a module-level straight-line plain `name = <function literal>` (`straight_line_assignments`), and the
  call provably runs after the last one (the temporal-dead-zone proof with a declarator standing at that write);
- a `const` bound to `f(…)`: `f` resolves exactly and runs nothing but one straight-line `return` of a function literal written in it, or
  of its own unwritten parameter whose argument is a function literal;
- a member of a module-level `const` closed literal that module code replaces: every reference of the receiver is in its file, a member
  call or a module-level straight-line `receiver.name = <function literal>`, no function of it reads `this`, and the call follows the last write.

Here each shape misses one condition, so the read stays uncertifiable; `forwarded-wrapper-exact` holds the proved shapes.

| component | missing | outcome |
|---|---|---|
| `InFunctionApp` | the write is in a function (`reset`) | uncertifiable |
| `OnBranchApp` | the write is on a branch | uncertifiable |
| `ByNameApp` | the write's value is a name, not a function literal | uncertifiable |
| `early()`'s call | module code makes it before the last write | uncertifiable |
| `LateApp` | (none: the component's call follows the last write) | SC5001 a violation |
| `NotedApp` | the factory calls `console.log` before returning | uncertifiable |
| `EitherApp` | the factory's return is conditional | uncertifiable |
| `MemberInFunctionApp` | the member is written in a function (`patch`) | uncertifiable |
| `MemberExportedApp` | the receiver is exported | uncertifiable |

Chrome: `fixtures/runtime-oracle/forwarded-wrapper-values`. Stubs: `forwarded-async-read`'s.
