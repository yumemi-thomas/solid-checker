// Reuse the exact audited rc.9 facts with the revision-separated collector.
import assert from 'node:assert/strict';
import {readFileSync,realpathSync} from 'node:fs';
import {dirname,join} from 'node:path';
import {packageRoot,read} from './catalog.mjs';
import {instrumentNativeReads} from './native-read-hook-v3.mjs';
export function nativeReadHook(){const transformed=[],refused=[];
  return {transformed,refused,transform(code,path){
    if(!path.endsWith('/dist/dev.js')&&!path.endsWith('/dist/dev-shared.js'))return null;
    let root;try{root=packageRoot(dirname(path),'@solidjs/signals');}catch{return null;}
    if(realpathSync(path)!==realpathSync(join(root,'dist',path.endsWith('/dev-shared.js')?'dev-shared.js':'dev.js')))return null;
    try{assert.equal(read(join(root,'package.json')).version,'2.0.0-rc.9');assert.equal(readFileSync(path,'utf8'),code);
      const result=instrumentNativeReads(code,path,{runtimeSpecifier:'/@fs'+new URL('./native-read-runtime-v4.mjs',import.meta.url).pathname});
      transformed.push(result.premise);if(result.store)transformed.push(result.store);return {code:result.code,map:result.map};
    }catch(error){refused.push({path,reason:error.message});return null;}
  }};
}
