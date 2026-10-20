# props-caller-witness

Caller-proven prop reactivity has two quantifiers: "some caller passes a reactive expression" needs one witness (monotone, sound in an open world); "every caller passes a static value" needs the complete caller set (a closed world). `WitnessedDynamic` (exported, dynamic witness) is a violation; `OnlyStaticWitness` (exported) is uncertifiable (a static witness never certifies an escaping component); `ClosedStatic` (complete local caller set) is silent; `WitnessedPerName` reports `shown` and leaves `hidden` uncertifiable (a witness is per prop name).
