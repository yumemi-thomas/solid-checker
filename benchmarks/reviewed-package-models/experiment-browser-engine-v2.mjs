// Owned local browser processes only; alternative engines grant no trace authority.
// Successful shutdown must not leave a referenced timeout keeping Node alive.
import assert from 'node:assert/strict';
import {spawn,execFileSync} from 'node:child_process';
import {openSync,closeSync,readFileSync} from 'node:fs';
import {createServer} from 'node:net';
import {join} from 'node:path';
import {setTimeout as delay} from 'node:timers/promises';
import {hash} from './catalog.mjs';

async function freePort(){
  const server=createServer();await new Promise((yes,no)=>{server.once('error',no);server.listen(0,'127.0.0.1',yes);});
  const port=server.address().port;await new Promise(yes=>server.close(yes));return port;
}
export async function launchExperimentBrowser({chromium,executablePath,engine,outputDirectory}){
  assert(['chromium','lightpanda'].includes(engine),'unknown browser engine');
  const identity={kind:engine,path:executablePath,sha256:hash(readFileSync(executablePath))};
  if(engine==='chromium'){
    const browser=await chromium.launch({executablePath,headless:true,timeout:15000});
    return {browser,identity:{...identity,version:browser.version()},shutdown:()=>browser.close()};
  }
  const version=execFileSync(executablePath,['version'],{encoding:'utf8',timeout:5000}).trim();
  const port=await freePort(),args=['serve','--host','127.0.0.1','--port',String(port),'--log-level','warn','--load-resources','stylesheet'];
  const log=join(outputDirectory,'lightpanda-server.log'),fd=openSync(log,'wx');
  const child=spawn(executablePath,args,{env:{...process.env,LIGHTPANDA_DISABLE_TELEMETRY:'1'},stdio:['ignore',fd,fd]});closeSync(fd);
  let processError;child.on('error',error=>processError=error);
  const exited=new Promise(yes=>child.once('exit',(code,signal)=>yes({code,signal})));
  const stop=async()=>{
    if(child.exitCode!==null||child.signalCode||processError)return;
    child.kill('SIGTERM');await Promise.race([exited,delay(2000,undefined,{ref:false})]);
    if(child.exitCode===null&&!child.signalCode){child.kill('SIGKILL');await exited;}
  };
  let browser,lastError;const started=Date.now();
  try{
    while(Date.now()-started<10000){
      if(processError)throw processError;
      assert(child.exitCode===null&&!child.signalCode,'Lightpanda exited before connection; see '+log);
      try{browser=await chromium.connectOverCDP(`ws://127.0.0.1:${port}`,{timeout:2000});break;}
      catch(error){lastError=error;await delay(100);}
    }
    assert(browser,lastError?.message??'Lightpanda connection timed out');
    return {browser,identity:{...identity,version,reportedVersion:browser.version(),args,log},shutdown:async()=>{
      try{await Promise.race([browser.close(),delay(2000,undefined,{ref:false})]);}finally{await stop();}
    }};
  }catch(error){await stop();throw error;}
}
