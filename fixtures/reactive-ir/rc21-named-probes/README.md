# SC1001: `latest` and `isPending` by name and over a pending store

`latest(m)` and `isPending(m)` call `m` in their own frame, and `isPending(() => p.a)`
over a pending store warns with no throw. Each is `strict-read-untracked`, where SC5001
named the wrong code or nothing reported (ADR 0633). Chrome, rc.21.
