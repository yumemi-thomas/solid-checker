Each form here looks like an identity wrapper and is not one, so the callback
it is handed stays unnamed and the checker stays silent (ADR 0346): a wrapper
that stores its callback or hands it to a scheduler (the callback runs later,
out of band); a wrapper that returns another function on one path; a wrapper
whose callback is returned on both branches but never on a straight-line
return (fail closed: nothing proves every completion returns it); a declaration
without a body; a member callee; and a local name that shadows an identity
wrapper. The positive forms are fixture `identity-wrapper-callback`.
