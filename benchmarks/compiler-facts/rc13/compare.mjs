import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {readFileSync,writeFileSync,existsSync,mkdirSync,readdirSync} from 'node:fs';
import {join,resolve} from 'node:path';
import {createRequire} from 'node:module';
import {spawnSync,execFileSync} from 'node:child_process';
import {isDeepStrictEqual} from 'node:util';

const require=createRequire(resolve('packages/cli/package.json'));
const ts=require('typescript');
const digest=bytes=>createHash('sha256').update(bytes).digest('hex');
export function compareRows(requests,upstream,fork){
  assert.equal(upstream.length,requests.length);assert.equal(fork.length,requests.length);
  assert.equal(new Set(requests.map(row=>row.id)).size,requests.length);
  const rows=[];
  for(let i=0;i<requests.length;i++){
    const request=requests[i],u=upstream[i],f=fork[i];
    assert.equal(u.id,request.id);assert.equal(f.id,request.id);
    assert(isDeepStrictEqual(f.plain,u.plain),`upstream output drift: ${request.id}`);
    const supported=['dom','ssr'].includes(request.options.generate);
    if(supported){
      assert(isDeepStrictEqual(f.traced,f.plain),`trace changed output: ${request.id}: ${f.traced.error??'emitted bytes differ'}`);
      if(f.plain.status==='compiled'){
        assert(f.trace,`missing trace: ${request.id}`);
        assert.equal(f.trace.version,3);
        assert.equal(f.trace.identity.compiler.upstream_revision,'5efaf260becb32293f2bcb4d32f8be72be6de674');
        assert.equal(f.trace.identity.compiler.implementation_revision,'c04c48779812d3d87166da3741c625748458c62f');
        assert.equal(f.trace.identity.source_sha256,digest(request.source));
        assert.equal(f.trace.identity.output_sha256,digest(f.plain.code));
        assert.equal(f.trace.identity.source_map_sha256,f.plain.map===null?null:digest(f.plain.map));
        const config=f.trace.identity.config;
        assert.equal(config.hoist_props,request.options.hoistProps);
        assert.equal(config.source_names_components,request.options.components);
        assert.equal(config.source_names_bindings,request.options.bindings);
      }
    }else {
      assert.equal(f.traced.status,'refused',`unsupported facts admitted: ${request.id}`);
      assert(f.trace==null,`unsupported facts emitted: ${request.id}`);
    }
    rows.push({id:request.id,compiled:f.plain.status==='compiled',supported,
      upstreamMatch:true,traceNeutral:supported?true:null,
      sourceOperations:f.trace?.sites.length??null,generatedOperations:f.trace?.generated_operations.length??null});
  }
  return rows;
}

function collectSources(root){
  const result=[],inputs=[];
  const add=(id,path,source)=>{result.push({id,source});inputs.push({id,path,sha256:digest(readFileSync(path))});};
  const fixtures=join(root,'packages/babel-plugin/test');
  for(const family of readdirSync(fixtures).filter(name=>name.startsWith('__')).sort()){
    for(const name of readdirSync(join(fixtures,family)).sort()){
      const path=join(fixtures,family,name,'code.js');
      if(existsSync(path))add(`fixture/${family}/${name}`,path,readFileSync(path,'utf8'));
    }
  }
  const path=join(root,'packages/compiler/__tests__/parity-probes.test.js'),text=readFileSync(path,'utf8');
  const ast=ts.createSourceFile(path,text,ts.ScriptTarget.Latest,true,ts.ScriptKind.JS);
  assert.equal(ast.parseDiagnostics.length,0);
  const declarations=[];
  function walk(node){if(ts.isVariableDeclaration(node)&&ts.isIdentifier(node.name)&&node.name.text==='cases')declarations.push(node);ts.forEachChild(node,walk);}
  walk(ast);assert.equal(declarations.length,1);
  const object=declarations[0].initializer;assert(ts.isObjectLiteralExpression(object));
  for(const property of object.properties){
    assert(ts.isPropertyAssignment(property));assert(ts.isStringLiteral(property.name));
    assert(ts.isNoSubstitutionTemplateLiteral(property.initializer));
    add(`probe/${property.name.text}`,path,property.initializer.text);
  }
  assert.equal(result.filter(row=>row.id.startsWith('fixture/')).length,95);
  assert.equal(result.filter(row=>row.id.startsWith('probe/')).length,270);
  assert.equal(new Set(result.map(row=>row.id)).size,result.length);
  return {sources:result,inputs};
}

function main(){
  const [outputArg]=process.argv.slice(2),output=resolve(outputArg);assert(!existsSync(output));
  const root=resolve('rust/target/compiler-rc13-upstream');
  const base=execFileSync('git',['rev-parse','HEAD'],{cwd:root,encoding:'utf8'}).trim();
  assert.equal(base,'5efaf260becb32293f2bcb4d32f8be72be6de674');
  const {sources,inputs}=collectSources(root);
  const common={generate:'dom',moduleName:'r-dom',builtIns:['For','Show'],staticMarker:'@once',
    hydratable:false,dev:false,sourceMap:true,serverComponents:false,hoistProps:true,components:false,bindings:false};
  const profiles=[
    {name:'baseline',settings:{sourceMap:false}},
    {name:'dom-hydratable-dev',settings:{hydratable:true,dev:true}},
    {name:'dom-source-names',settings:{components:true,bindings:true}},
    {name:'ssr',settings:{generate:'ssr'}},
    {name:'ssr-hydratable-dev',settings:{generate:'ssr',hydratable:true,dev:true}},
    {name:'ssr-server-components',settings:{generate:'ssr',hydratable:true,serverComponents:true,components:true,bindings:true}},
    {name:'ssr-no-hoist',settings:{generate:'ssr',hoistProps:false}},
    {name:'universal',settings:{generate:'universal'}},
    {name:'dynamic',settings:{generate:'dynamic'}},
  ];
  const requests=sources.flatMap(source=>profiles.map(profile=>({id:`${source.id}|${profile.name}`,source:source.source,options:{...common,...profile.settings}})));
  mkdirSync(output,{recursive:true});
  writeFileSync(join(output,'requests.json'),JSON.stringify(requests));
  const binaries=['compiler-rc13-upstream-probe','compiler-rc13-fork-probe'].map(name=>resolve('rust/target/compiler-rc13-build/debug',name));
  const outputs=binaries.map((binary,index)=>{
    const run=spawnSync(binary,[],{input:JSON.stringify(requests),encoding:'utf8',timeout:60000,maxBuffer:256*1024*1024});
    writeFileSync(join(output,index?'fork.json':'upstream.json'),run.stdout??'');
    writeFileSync(join(output,index?'fork-stderr.log':'upstream-stderr.log'),run.stderr??'');
    assert.equal(run.status,0,run.stderr);return JSON.parse(run.stdout);
  });
  const rows=compareRows(requests,...outputs);
  const baseline=outputs[0].filter(row=>row.id.endsWith('|baseline')).map(row=>{
    const id=row.id.slice(0,-'|baseline'.length);
    return row.plain.status==='compiled'?`${id}\tok\t${Buffer.from(row.plain.code).toString('hex')}\n`:`${id}\treject\n`;
  }).join('');
  writeFileSync(join(output,'upstream-transform-output-baseline.txt'),baseline);
  for(const input of inputs)assert.equal(digest(readFileSync(input.path)),input.sha256);
  const report={authority:false,certification:false,upstream:base,finishedAt:new Date().toISOString(),
    summary:{sources:sources.length,profiles:profiles.length,comparisons:rows.length,upstreamMatches:rows.length,
      traceComparisons:rows.filter(row=>row.supported).length,unsupportedFactRefusals:rows.filter(row=>!row.supported).length},
    inputs,binaries:binaries.map(path=>({path,sha256:digest(readFileSync(path))})),rows,
    limits:['This compares compiler output and fact neutrality; it does not run applications or establish package/runtime compatibility.','Generated operations are positive observations with partial enumeration.']};
  writeFileSync(join(output,'results.json'),JSON.stringify(report,null,2)+'\n');console.log(JSON.stringify(report.summary));
}
if(process.argv[1]&&resolve(process.argv[1])===new URL(import.meta.url).pathname)main();
