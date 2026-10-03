// Observe unchanged production fixtures. A hidden map is usable only when its
// generated code is byte-identical to the original server's served client bundle.
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {closeSync,existsSync,mkdirSync,openSync,readFileSync,realpathSync,writeFileSync} from 'node:fs';
import {dirname,join,resolve} from 'node:path';
import {pathToFileURL,fileURLToPath} from 'node:url';
import {setTimeout as delay} from 'node:timers/promises';
import {closurePins,hash,packageRoot} from './catalog.mjs';

const [runArg,outArg,browserPath]=process.argv.slice(2),runPath=resolve(runArg),output=resolve(outArg),run=JSON.parse(readFileSync(runPath));
assert(browserPath&&existsSync(browserPath)&&!existsSync(output));assert(run.finishedAt&&run.originalAndCopiedInputsUnchanged);
assert.equal(run.typing.exitCode,0);const root=run.clone,variant=run.variant,origin='http://127.0.0.1:4174';mkdirSync(output,{recursive:true});
function authenticate(){for(const pin of run.inputs.application){assert.equal(hash(readFileSync(join(run.source,pin.path))),pin.sha256);assert.equal(hash(readFileSync(join(root,pin.path))),pin.sha256);}for(const pkg of run.inputs.packages)assert.deepEqual(closurePins(pkg.root),pkg.pins);}
authenticate();
const tooling=resolve('rust/target/app-import-metric/apps/helge-dev'),load=(name,entry,from=root)=>import(pathToFileURL(join(packageRoot(from,name),entry)));
const {build}=await load('vite','dist/node/index.js'),{default:solid}=await load('vite-plugin-solid','index.mjs');
const {chromium}=await load('playwright','index.mjs',tooling),{TraceMap,originalPositionFor}=await load('@jridgewell/trace-mapping','dist/trace-mapping.mjs',tooling);
const alias={'solid-js/web':'@solidjs/web',...(variant==='candidate'?{
  './kobalte/Dialog':resolve(root,'src/ui/kobalte/__tests__/alpha/Dialog.tsx'),
  './kobalte/Select':resolve(root,'src/ui/kobalte/__tests__/alpha/Select.tsx'),
  '../../src/ui/kobalte/Dialog':resolve(root,'src/ui/kobalte/__tests__/alpha/Dialog.tsx'),
  '../../src/ui/kobalte/Select':resolve(root,'src/ui/kobalte/__tests__/alpha/Select.tsx'),
}:{})};
// Rollup also uses the working directory for region labels. Match the original
// fixture process so the independently mapped build must match every served byte.
const originalCwd=process.cwd();let result;
try{
  process.chdir(root);
  result=await build({configFile:false,root,plugins:[solid({ssr:true})],logLevel:'silent',resolve:{alias},define:{'process.env.NODE_ENV':'"production"'},ssr:{noExternal:true},
    build:{write:false,minify:false,sourcemap:'hidden',lib:{entry:resolve(root,'e2e/fixtures/controls-client.tsx'),formats:['es']}}});
}finally{process.chdir(originalCwd);}
const chunk=(Array.isArray(result)?result[0]:result).output.find(item=>item.type==='chunk');assert(chunk?.map);
const map=typeof chunk.map==='string'?JSON.parse(chunk.map):chunk.map,trace=new TraceMap(map),sourceBase=join(root,'dist');
writeFileSync(join(output,'mapped-client.js'),chunk.code);writeFileSync(join(output,'client.map.json'),JSON.stringify(map));
function owner(path){
  for(let folder=dirname(path);;folder=dirname(folder)){
    const manifest=join(folder,'package.json');
    if(existsSync(manifest)){const value=JSON.parse(readFileSync(manifest));return {name:value.name,version:value.version,root:realpathSync(folder),manifest,manifestSha256:hash(readFileSync(manifest)),role:folder===root?'application':'package-artifact',staticDispatch:'open'};}
    if(dirname(folder)===folder)return null;
  }
}
const sources=map.sources.map((name,index)=>{
  if(name.startsWith('\0')||/^[a-z]+:\/\//.test(name))return {name,status:'unavailable'};
  const path=resolve(sourceBase,map.sourceRoot??'',name),content=map.sourcesContent?.[index];
  if(!existsSync(path)||typeof content!=='string')return {name,path,status:'unavailable'};
  const bytes=readFileSync(path,'utf8');if(bytes!==content)return {name,path,status:'bytes-differ'};
  return {name,path,sourceSha256:hash(bytes),status:'exact',owner:owner(path)};
});
const report={authority:false,certification:false,startedAt:new Date().toISOString(),variant,
  scope:'Observed unchanged retained production SSR/client fixture; map binding requires exact client bytes and published source bytes. Source locations are observed links, not causal responsibility or static dispatch proof.',
  inputs:{run:{path:runPath,sha256:hash(readFileSync(runPath))},runner:{path:fileURLToPath(import.meta.url),sha256:hash(readFileSync(fileURLToPath(import.meta.url)))},
    tools:['playwright','@jridgewell/trace-mapping'].map(name=>{const root=packageRoot(tooling,name);return {name,root,pins:closurePins(root)};})},
  map:{file:join(output,'client.map.json'),sha256:hash(JSON.stringify(map)),codeSha256:hash(chunk.code),sources},
  server:null,diagnostics:[],blockedRequests:[],clientByteIdentity:null,snapshot:null};
const save=()=>writeFileSync(join(output,'results.json'),JSON.stringify(report,null,2)+'\n');save();
let browser,context,child;const log=openSync(join(output,'server.log'),'wx');
try{
  let occupied=false;try{const response=await fetch(origin,{signal:AbortSignal.timeout(500)});occupied=!!response;}catch{}
  assert(!occupied,'The fixed original fixture port must be free');
  child=spawn(process.execPath,['e2e/fixtures/server.ts'],{cwd:root,env:{...process.env,KOBALTE_COMPATIBILITY:variant==='candidate'?'1':'0',SOLID_VIRTUAL_COMPATIBILITY:'0'},detached:true,stdio:['ignore',log,log]});
  child.once('error',error=>{report.server={error:error.message};save();});
  const until=Date.now()+30000;let ready=false;
  while(Date.now()<until){assert.equal(child.exitCode,null,'Original server exited before readiness');try{const response=await fetch(origin,{signal:AbortSignal.timeout(500)});if(response.ok){ready=true;break;}}catch{}await delay(100);}
  assert(ready,'Original server did not become ready');
  browser=await chromium.launch({executablePath:browserPath,headless:true,timeout:15000});report.browser=browser.version();context=await browser.newContext();
  let release;const gate=new Promise(resolveGate=>{release=resolveGate;});
  await context.route('**/*',async route=>{const url=new URL(route.request().url());if(url.origin!==origin){report.blockedRequests.push(url.href);return route.abort();}if(url.pathname==='/client.js')await gate;return route.continue();});
  const page=await context.newPage();page.setDefaultTimeout(8000);
  page.on('console',message=>{if(['warning','error'].includes(message.type()))report.diagnostics.push({kind:'console',severity:message.type(),text:message.text(),location:message.location()});});
  page.on('pageerror',error=>report.diagnostics.push({kind:'pageerror',name:error.name,message:error.message,stack:error.stack}));
  const clientResponse=page.waitForResponse(response=>new URL(response.url()).pathname==='/client.js');
  const response=await page.goto(origin,{waitUntil:'commit'});await page.getByRole('button',{name:'Edit preferences',exact:true}).waitFor({state:'visible'});
  const originalRoot=await page.locator('#root').elementHandle(),originalTrigger=await page.getByRole('button',{name:'Edit preferences',exact:true}).elementHandle();
  report.response={status:response.status(),csp:response.headers()['content-security-policy']};
  release();const served=await (await clientResponse).text();writeFileSync(join(output,'served-client.js'),served);report.clientByteIdentity={servedSha256:hash(served),mappedSha256:hash(chunk.code),matched:served===chunk.code};
  assert(report.clientByteIdentity.matched,'The map build changed the served bundle; attribution remains open');
  try{await page.locator('html').waitFor({state:'attached'});await page.waitForFunction(()=>document.documentElement.dataset.hydrated==='true',{},{timeout:5000});}catch(error){report.hydrationWait={message:error.message};}
  await delay(100);
  report.snapshot=await page.evaluate(()=>({hydrated:document.documentElement.dataset.hydrated??null,text:document.body.innerText,inlineStyleAttributes:document.querySelectorAll('[style]').length,selectCount:document.querySelectorAll('select[name=role]').length}));
  report.snapshot.rootReused=await page.locator('#root').evaluate((node,original)=>node===original,originalRoot);
  const trigger=page.getByRole('button',{name:'Edit preferences',exact:true});report.snapshot.triggerReused=await trigger.count()===1?await trigger.evaluate((node,original)=>node===original,originalTrigger):false;
  function mapped(frame){
    if(new URL(frame.url).origin!==origin||new URL(frame.url).pathname!=='/client.js')return {...frame,status:'outside-client-bundle'};
    const position=originalPositionFor(trace,{line:frame.line,column:frame.column-1}),source=sources.find(row=>row.name===position.source);
    if(!source||source.status!=='exact'||!position.line)return {...frame,status:'unavailable',source:position.source};
    const text=readFileSync(source.path,'utf8'),line=text.split('\n')[position.line-1];
    if(line===undefined||position.column<0||position.column>line.length)return {...frame,status:'invalid-source-position'};
    return {...frame,status:'exact',original:{path:source.path,line:position.line,column:position.column+1,name:position.name,sourceSha256:source.sourceSha256,owner:source.owner}};
  }
  for(const diagnostic of report.diagnostics){
    const locations=[];
    if(diagnostic.location?.url&&diagnostic.location.url.startsWith(origin))locations.push({url:diagnostic.location.url,line:diagnostic.location.lineNumber+1,column:diagnostic.location.columnNumber+1});
    for(const line of (diagnostic.stack??'').split('\n')){const match=line.match(/(https?:\/\/[^\s)]+):(\d+):(\d+)\)?$/);if(match)locations.push({url:match[1],line:Number(match[2]),column:Number(match[3])});}
    diagnostic.frames=locations.map(mapped);
  }
  assert.equal(child.exitCode,null,'Original server exited during observation');save();
}catch(error){report.failure={message:error.message,stack:error.stack};save();throw error;}
finally{
  await context?.close();await browser?.close();
  if(child?.pid){try{process.kill(-child.pid,'SIGTERM');}catch(error){if(error.code!=='ESRCH')throw error;}if(child.exitCode===null)await new Promise(resolveClose=>child.once('close',resolveClose));}
  closeSync(log);report.server={pid:child?.pid,exitCode:child?.exitCode,signal:child?.signalCode,originalInput:'e2e/fixtures/server.ts'};
}
authenticate();report.finishedAt=new Date().toISOString();save();console.log(JSON.stringify({variant,clientBytesMatched:report.clientByteIdentity?.matched,hydrated:report.snapshot?.hydrated,diagnostics:report.diagnostics.length,mappedFrames:report.diagnostics.flatMap(row=>row.frames??[]).filter(row=>row.status==='exact').length,sourceFiles:sources.filter(row=>row.status==='exact').length}));
