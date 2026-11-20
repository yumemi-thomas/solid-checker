# event-delivery-spellings (ADR 1133, round 1 of the Fable review)

The delivery scan (`event_delivery::scan_delivery_hazard`, ADR 1125) withholds every event proof from a program in which some code may
make a handler run other than as a DOM delivery that sets `window.event`. Each variant is one spelling of such code that the first cut of
ADR 1133 read as no hazard (the review's F1-F5), next to a handler whose read stays uncertifiable; this project holds the neighbours of
every spelling (the closest legal forms), and the read of `Light` stays proved.

- F1 (regression): `Reflect.get` hands back the member itself under any key (`reflect-get-constructor`, `-dispatch`, `-timer`); only
  `Reflect.apply`, `construct` and `has` are no hazard.
- F2 (regression): the sinks `Sinks::call` matches by a call's spelling, read as a value (`reflect-apply-insert-adjacent-html`,
  `call-insert-adjacent-html`, `reflect-apply-document-write`, `reflect-apply-create-element-script`).
- F3: a dispatcher under a key the program builds or taken out by an assignment pattern (`dispatch-built-key`,
  `dispatch-assignment-pattern`).
- F4: `JSON` and `Object` by a member of the global object, an alias, a computed member or a pattern (`json-*`, `object-*`).
- F5: a `javascript:` URL or markup by a route the first cut did not read (`location-member-*`, `href-built-key`, `attribute-node`,
  `svg-base-val`, `protocol`, `href-entity`, `srcdoc-entity`, `inner-html-entity`, `open-pattern`).
- Adjacent spellings (found by probing, not by the review): the attribute node out of the `attributes` collection (`attributes-*`),
  `execCommand("insertHTML")` (`exec-command`), `parseFromString` (`dom-parser`), and a window or a document behind a `const` alias (`window-alias-*`,
  `document-alias-create-element`).
- F6 (`variants/plain-data-key-uses`, a legal project): K4 judges the key alone; a direct call, a call through a name and `.call` of a
  member under a plain data key are alike inert.
