import {execFileSync} from 'node:child_process';
export function processTree(text,root){
  const rows=text.trim().split('\n').filter(Boolean).map(line=>{
    const match=line.trim().match(/^(\d+)\s+(\d+)\s+(\d+)\s+(.*)$/);if(!match)throw Error('Malformed process memory sample');
    return {pid:Number(match[1]),parent:Number(match[2]),rssBytes:Number(match[3])*1024,command:match[4]};
  });
  const owned=new Set([root]);let changed=true;
  while(changed){changed=false;for(const row of rows)if(owned.has(row.parent)&&!owned.has(row.pid)){owned.add(row.pid);changed=true;}}
  const tree=rows.filter(row=>owned.has(row.pid)),browser=tree.filter(row=>/lightpanda|Google Chrome|chrome-mac|Chromium/i.test(row.command));
  return {processes:tree.length,totalRssBytes:tree.reduce((n,row)=>n+row.rssBytes,0),browserProcesses:browser.length,browserRssBytes:browser.reduce((n,row)=>n+row.rssBytes,0)};
}
export function sampleMemory(root){return processTree(execFileSync('/bin/ps',['-axo','pid=,ppid=,rss=,comm='],{encoding:'utf8',maxBuffer:4*1024*1024}),root);}
