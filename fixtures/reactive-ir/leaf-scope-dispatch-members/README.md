# leaf-scope-dispatch-members

ADR 0705. Members the dispatch list lacked: `form.submit`, `reportError`, `replaceWith`, `replaceChildren` and a navigation
(`location.reload`) dispatch on their caller's stack (Chrome, rc.21), so an `onSettled` body that calls one is not proven free
of a forbidden call: uncertifiable. A project method named `submit` and an inert `lib.dom` member stay clean.
