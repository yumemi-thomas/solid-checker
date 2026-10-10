// Process-test transport twin for opted-in ADR 0220. This controlled worker
// supplies exact canonical rows; it does not model or execute a Vite resolver.
import { join } from 'node:path';
let input = '';
for await (const chunk of process.stdin) input += chunk;
const request = JSON.parse(input);
const choice = process.env.SOLID_CHECKER_TEST_RESOLVER_CHOICE;
const rows = request.loads.map(load => {
  const target = load.specifier === '@solidjs/web'
    ? choice === 'client' ? 'node_modules/@solidjs/web/dist/web.js'
      : choice === 'server' ? 'node_modules/@solidjs/web/dist/server.js'
      : choice === 'shadow' ? 'src/shadow.js'
      : choice === 'token-shadow' ? '../shadow-base/@solidjs/web.js' : null
    : load.specifier === 'reactive-package' ? 'node_modules/reactive-package/index.js' : null;
  const path = target && join(request.root, target);
  return { id: load.id, outcome: path
    ? { kind: 'file', path, physicalPath: path }
    : { kind: 'unknown' } };
});
console.log(JSON.stringify({ protocol: request.protocol, status: 'answered', rows }));
