# Props and store reads in anonymous callbacks (ADR 0323)

A props or store read in an anonymous function literal is classified exactly like its signal twin where the callback reaches a project callee, inline standard-library callback or IIFE; elsewhere (component attributes, unknown callees, timers, `Promise.then`, stored literals, non-props parameters) it takes the role its signal twin takes (four become uncertifiable). Options-object members: ADR 0321 resolves dispatch, not timing. Published rc.13 declarations copied from `standard-runtime-configuration`.
