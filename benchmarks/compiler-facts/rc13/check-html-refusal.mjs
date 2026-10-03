import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {spawnSync} from 'node:child_process';
import {createRequire} from 'node:module';
import {resolve} from 'node:path';
import {writeFileSync} from 'node:fs';

const [checkerArg,outputArg]=process.argv.slice(2),checker=resolve(checkerArg);
const require=createRequire(resolve('rust/target/compiler-rc13-release-v1/node_modules/@solidjs/compiler/index.js'));
const compiler=require('./index.js'),results=[];
for(const [id,source,accepted] of [
  ['legacy-keygen','const view = <div><keygen>{count()}</keygen></div>;',false],
  ['menuitem','const view = <menuitem>{count()}</menuitem>;',true],
  ['nested-void','const view = <div><br>{count()}</br></div>;',true],
  ['root-void','const view = <br>{count()}</br>;',true],
  ['ordinary-child','const view = <div>{count()}</div>;',true],
]){
  const request={protocol:1,nonce:id,snapshotRoot:'sha256:'+'1'.repeat(64),demandGraphRoot:'sha256:'+'2'.repeat(64),demandId:'sha256:'+'3'.repeat(64),
    analysis:{compilerFactsProtocol:2,path:'input.tsx',source,sourceHash:'sha256:'+createHash('sha256').update(source).digest('hex'),
      compilerOptions:{moduleName:'dom',generate:'dom',hydratable:false,dev:false,builtIns:[]}}};
  const child=spawnSync(checker,['--internal-compiler-certification-session'],{input:JSON.stringify(request),encoding:'utf8'});
  assert(!child.error);assert.equal(child.status===0,accepted,child.stderr);
  let published;
  try{published=compiler.transform(source,{filename:'input.tsx',moduleName:'dom',generate:'dom',builtIns:[]});assert(accepted,'published compiler unexpectedly accepted keygen');}
  catch(error){assert(!accepted,error.message);assert(error.message.includes('HTML provided is malformed'));}
  if(accepted){
    const response=JSON.parse(child.stdout);
    assert.equal(response.compilerIdentity,'solid-v2:trace3:c04c48779812d3d87166da3741c625748458c62f');
    assert.equal(response.output,published.code);
    results.push({id,accepted:true,compiledOutputMatched:true});
  }else{
    assert(child.stderr.includes('HTML provided is malformed'));assert.equal(child.stdout,'');
    results.push({id,accepted:false,compilerFactsRefused:true});
  }
}
writeFileSync(resolve(outputArg),JSON.stringify({authority:false,certification:false,results},null,2)+'\n');
console.log(JSON.stringify(results));
