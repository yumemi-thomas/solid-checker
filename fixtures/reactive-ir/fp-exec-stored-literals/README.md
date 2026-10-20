# fp-exec-stored-literals

**Claim.** Code in a component is claimed as body-time only where facts prove it runs during the body: the body, an IIFE, a standard-library inline callback, a project function calling its parameter during the call, a control-flow render callback, a named helper every reference of which is a proven body-time call. Stored functions, callbacks handed to consumers not proven to invoke them, and helpers called from handlers/tracked attributes are `SC1001` uncertifiable or absent, never proven untracked reads. Stub: `createSignal` verbatim from rc.9.
