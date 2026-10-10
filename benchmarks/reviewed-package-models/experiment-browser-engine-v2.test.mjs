import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtempSync,writeFileSync,chmodSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {spawn} from 'node:child_process';
import {performance} from 'node:perf_hooks';

test('successful owned-process shutdown does not keep Node alive for its two-second deadlines',async()=>{
  const root=mkdtempSync(join(tmpdir(),'solid-checker-browser-shutdown-')),executable=join(root,'fake-lightpanda');
  writeFileSync(executable,'#!'+process.execPath+'\nif(process.argv[2]==="version"){console.log("test-only");process.exit(0);}setInterval(()=>{},1000);\n');chmodSync(executable,0o755);
  const runner=join(root,'runner.mjs'),engine=new URL('./experiment-browser-engine-v2.mjs',import.meta.url).href;
  writeFileSync(runner,`import {launchExperimentBrowser} from ${JSON.stringify(engine)};
    const owned=await launchExperimentBrowser({engine:'lightpanda',executablePath:${JSON.stringify(executable)},outputDirectory:${JSON.stringify(root)},chromium:{connectOverCDP:async()=>({version:()=> 'test-only',close:async()=>{}})}});
    await owned.shutdown();console.log('closed');`);
  const started=performance.now(),child=spawn(process.execPath,[runner],{stdio:['ignore','pipe','pipe']});let output='',errors='';
  child.stdout.on('data',data=>output+=data);child.stderr.on('data',data=>errors+=data);
  const status=await new Promise((yes,no)=>{child.once('error',no);child.once('exit',yes);});
  assert.equal(status,0,errors);assert.match(output,/closed/);assert(performance.now()-started<1500,'a successful shutdown left a referenced deadline alive');
});
