// A long-lived Vitest host for the fast path (fast-daemon.mjs). Started once inside the install's work
// directory, it keeps the Vite server, the Solid plugin and its transform cache alive, and runs the test
// files it is told to, one request at a time. Isolation is Vitest's own: every test file gets a fresh
// worker environment (`isolate: true` in vitest.config.mjs), so a program starts with a new Happy DOM
// window and a new module graph, whatever ran before it.
//
// Protocol: one JSON object per stdin line, `{ id, tests: [absolute test file, ...] }`; one reply line per
// request on stdout, `@@{ id, ok, error? }` (the prefix keeps program output out of the way).
import { createInterface } from 'node:readline';
import { createVitest } from 'vitest/node';

const reply = (value) => process.stdout.write(`@@${JSON.stringify(value)}\n`);
const vitest = await createVitest({ root: process.cwd(), config: 'vitest.config.mjs', watch: false, reporters: [{}] });
reply({ ready: true });
for await (const line of createInterface({ input: process.stdin })) {
  if (!line.trim()) continue;
  const request = JSON.parse(line);
  try {
    const project = vitest.getRootProject();
    await vitest.runTestSpecifications(request.tests.map((file) => project.createSpecification(file)));
    reply({ id: request.id, ok: true });
  } catch (error) {
    reply({ id: request.id, ok: false, error: String(error?.stack ?? error).slice(0, 800) });
  }
}
await vitest.close();
process.exit(0);
