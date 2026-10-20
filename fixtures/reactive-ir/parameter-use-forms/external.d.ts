// Values and functions with no body in the program.
declare const opaque: { run: () => void };
declare const partial: { run?: () => void; other?: number };
declare function makeOpaque(): { run: () => void };
declare function mutate(target: object): void;
