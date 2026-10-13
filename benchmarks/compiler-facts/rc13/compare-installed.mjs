import assert from 'node:assert/strict';
import {readFileSync,writeFileSync,existsSync,mkdirSync} from 'node:fs';
import {resolve,join} from 'node:path';
import {createRequire} from 'node:module';
import {createHash} from 'node:crypto';
import {isDeepStrictEqual} from 'node:util';

const [releaseArg,comparisonArg,outputArg]=process.argv.slice(2);
const release=resolve(releaseArg),comparison=resolve(comparisonArg),output=resolve(outputArg);
assert(!existsSync(output));mkdirSync(output,{recursive:true});
const metadata=JSON.parse(readFileSync(join(release,'release.json')));
assert.equal(metadata.version,'2.0.0-rc.13');
const root=join(release,'node_modules/@solidjs/compiler');
assert(!process.env.SOLID_COMPILER_NATIVE&&!process.env.NAPI_RS_FORCE_WASI);
const require=createRequire(join(root,'index.js')),compiler=require(root);
const requests=JSON.parse(readFileSync(join(comparison,'requests.json')));
const sourceOutputs=JSON.parse(readFileSync(join(comparison,'upstream.json')));
const results=[];
for(let i=0;i<requests.length;i++){
  const request=requests[i],source=sourceOutputs[i];assert.equal(source.id,request.id);
  const {components,bindings,...options}=request.options;
  let actual;
  try{
    const value=compiler.transform(request.source,{...options,filename:'/compiler-rebase/input.tsx',sourceNames:{components,bindings}});
    actual={status:'compiled',code:value.code,map:value.map??null,css:value.css??null,cssHash:value.cssHash??null};
  }catch(error){actual={status:'refused',error:error.message};}
  results.push({id:request.id,actual});
  writeFileSync(join(output,'progress.json'),JSON.stringify({last:request.id,completed:results.length}));
  assert.equal(actual.status,source.plain.status,`published artifact status drift: ${request.id}`);
  if(actual.status==='compiled')assert(isDeepStrictEqual(actual,source.plain),`published artifact output drift: ${request.id}`);
  else assert.equal(actual.error,source.plain.error,`published artifact diagnostic drift: ${request.id}`);
}
const binaries=Object.keys(require.cache).filter(path=>path.endsWith('.node'));
assert.equal(binaries.length,1);assert(binaries[0].startsWith(join(release,'node_modules/@solidjs/compiler-darwin-arm64/') ));
const report={authority:false,certification:false,version:metadata.version,revision:metadata.revision,
  summary:{comparisons:results.length,outputAndDiagnosticMatches:results.length},
  native:{path:binaries[0],sha256:createHash('sha256').update(readFileSync(binaries[0])).digest('hex')},results};
writeFileSync(join(output,'results.json'),JSON.stringify(report)+'\n');console.log(JSON.stringify(report.summary));
