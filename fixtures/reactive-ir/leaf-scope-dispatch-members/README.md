# leaf-scope-dispatch-members

ADR 0705: `form.submit`, `reportError`, `replaceWith`, `replaceChildren` and `location.reload` dispatch on the caller's stack
(Chrome, rc.21), so an `onSettled` body calling one is uncertifiable. A project `submit` and inert members stay clean.
