// Avoid constructing a TypeScript program when V2 cannot enroll a read.
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {hash} from './catalog.mjs';
import {mayHaveNativeCallbackRead} from './async-read-prefilter-v1.mjs';
import projectReadPlugin,{transformProjectReads} from './async-read-transform-v3.mjs';

export function transformCandidateProjectReads(code,path,session){
  if(mayHaveNativeCallbackRead(code,path))return transformProjectReads(code,path,session);
  return {code,map:null,sites:[],open:[],session:{skipped:true,reason:'no syntax eligible for the supported native-read selector'}};
}
export default function candidateProjectReadPlugin(){
  const base=projectReadPlugin();let project;
  return {...base,get session(){return base.session;},configResolved(config){project=config.root;base.configResolved(config);},
    transform(code,id){const path=id.split('?')[0];
      if(!path.startsWith(project+'/src/')||!path.endsWith('.tsx')||mayHaveNativeCallbackRead(code,path))return base.transform(code,id);
      try{assert.equal(readFileSync(path,'utf8'),code,'Another transform changed the original consumer before observation');
        base.transformed.push({path,sourceSha256:hash(code),transformedSha256:hash(code),sites:[],open:[],session:{skipped:true,reason:'no eligible read syntax'}});
        return {code,map:null};
      }catch(error){base.refused.push({path,reason:error.message});return null;}}
  };
}
