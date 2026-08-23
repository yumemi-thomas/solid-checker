// A side-effect import of a conditional `imports` branch. Nothing in this file
// reads a value from it, so nothing about the entrypoint's summaries depends on
// resolving it -- which is exactly why the record has to say the branch exists.
import "#platform";

export const thing = 1;
