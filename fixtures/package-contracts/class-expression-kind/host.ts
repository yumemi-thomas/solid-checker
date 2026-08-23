// An untyped host object -- what a published `.js` artifact importing a
// dependency with no type declarations actually has. Everything reached
// through it is `any`, and Type Facts answers `Callability::Unknown`: no
// closed domain, so neither `kind` is provable.
declare const host: any;

const fromHost = host.create();

export { fromHost };
