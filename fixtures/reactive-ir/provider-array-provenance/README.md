# Returned provider array provenance

An exact factory returns one array of closed object literals with exact synchronous arrow queries; `map`/`forEach` (standard-library identity) with an inline arrow whose body invokes one member of its element parameter. Body/eager/apply consumers read the original signal slots; tracked/inert queries and type-only `SourceAccessor` labels stay clean/silent (ADR 0294); aliased/escaped/replaced arrays, stored or async callbacks, element aliases and multi-call helpers withhold transport. Declarations copy `accessor-provenance`'s published rc.13 signatures.
