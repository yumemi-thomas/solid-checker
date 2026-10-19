# Callback producer

Pins the exported-surface answer for callback timing, which needs no package: `runInline` and `runTracked` are provable from their own bodies, while `runDeferred` runs its callback inside `onSettled`'s conditional leaf owner, a role the callback timing summary cannot express, so the exported surface stays one `SC9012` uncertifiable result instead of publishing only "deferred".
