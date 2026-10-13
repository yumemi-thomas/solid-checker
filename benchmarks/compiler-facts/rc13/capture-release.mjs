import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {existsSync,mkdirSync,writeFileSync,readFileSync} from 'node:fs';
import {resolve,join} from 'node:path';
import {execFileSync} from 'node:child_process';

const [outputArg]=process.argv.slice(2),output=resolve(outputArg);
assert(!existsSync(output));mkdirSync(output,{recursive:true});
const revision='5efaf260becb32293f2bcb4d32f8be72be6de674',version='2.0.0-rc.13';
const packages=[];
for(const name of ['solid-js','@solidjs/compiler','@solidjs/web','@solidjs/signals','@solidjs/compiler-darwin-arm64']){
  const response=await fetch(`https://registry.npmjs.org/${encodeURIComponent(name)}`);
  assert(response.ok);const packument=await response.json(),metadata=packument.versions[version];
  assert(metadata);assert.equal(metadata.gitHead,revision);
  assert.equal(packument['dist-tags'].next,version,'the latest release changed during preparation');
  const row={name,version,published:packument.time[version],tags:packument['dist-tags'],gitHead:metadata.gitHead,dist:metadata.dist};
  if(name.startsWith('@solidjs/compiler')){
    const response=await fetch(metadata.dist.tarball);assert(response.ok);
    const bytes=Buffer.from(await response.arrayBuffer());
    const integrity=`sha512-${createHash('sha512').update(bytes).digest('base64')}`;
    assert.equal(integrity,metadata.dist.integrity,'published tarball integrity');
    const archive=join(output,name.endsWith('arm64')?'native.tgz':'compiler.tgz');writeFileSync(archive,bytes);
    const entries=execFileSync('tar',['-tzf',archive],{encoding:'utf8'}).trim().split('\n');
    assert(entries.every(path=>path.startsWith('package/')&&!path.split('/').includes('..')));
    const root=join(output,'node_modules',name);mkdirSync(root,{recursive:true});
    execFileSync('tar',['-xzf',archive,'--strip-components=1','-C',root]);
    assert.equal(JSON.parse(readFileSync(join(root,'package.json'))).version,version);
    row.artifact={archive,sha256:createHash('sha256').update(bytes).digest('hex'),root};
  }
  packages.push(row);
}
writeFileSync(join(output,'release.json'),JSON.stringify({collectedAt:new Date().toISOString(),packages,revision,version},null,2)+'\n');
console.log(JSON.stringify({version,revision,packages:packages.map(row=>({name:row.name,published:row.published}))}));
