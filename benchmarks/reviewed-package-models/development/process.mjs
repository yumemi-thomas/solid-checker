import {spawn} from 'node:child_process';
import {openSync,closeSync} from 'node:fs';
import {performance} from 'node:perf_hooks';
import {sampleMemory} from './memory.mjs';
export async function execute(args,log,{memory=false}={}){
  const fd=openSync(log,'wx'),started=performance.now(),samples=[],errors=[];let child,timer,status;
  try{
    status=await new Promise((yes,no)=>{
      child=spawn(process.execPath,args,{stdio:['ignore',fd,fd]});
      if(memory)timer=setInterval(()=>{try{samples.push({elapsedMs:performance.now()-started,...sampleMemory(child.pid)});}catch(error){errors.push(error.message);}},100);
      child.once('error',no);child.once('exit',(code,signal)=>yes({code,signal}));
    });
  }finally{clearInterval(timer);closeSync(fd);}
  return {...status,wallMs:performance.now()-started,log,...(memory?{memory:{scope:'100 ms samples of runner and owned descendants; summed RSS includes shared pages; sampled maxima, not unique physical memory or continuous peaks',samples,errors,
    maxTotalRssBytes:Math.max(0,...samples.map(row=>row.totalRssBytes)),maxBrowserRssBytes:Math.max(0,...samples.map(row=>row.browserRssBytes))}}:{})};
}
