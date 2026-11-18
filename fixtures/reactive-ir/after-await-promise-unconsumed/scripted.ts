// Uncertifiable (third review of 1090 G1, H2): `export {}` is this file's only module syntax, which esbuild erases; loaded as a classic script,
// `fire` is a global another script may call where its promise is kept.
export {};
declare function report(): void;
const pause = () => Promise.resolve();
const fire = async () => {
  await pause();
  report();
};
fire();
