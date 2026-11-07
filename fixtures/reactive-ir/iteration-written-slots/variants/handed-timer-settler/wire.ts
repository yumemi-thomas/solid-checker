const sleep = (ms: number) => new Promise((done) => setTimeout(done, ms));
const fail = (ms: number) =>
  new Promise((_done, reject) => window.setTimeout(reject, ms));
void sleep(1);
void fail(1).catch(() => {});
export {};
