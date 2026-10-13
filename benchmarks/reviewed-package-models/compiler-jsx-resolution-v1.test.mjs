// Regression checks against the application's actual published RC.9 types.
import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {existsSync,mkdirSync,readFileSync,writeFileSync} from 'node:fs';
import {join,resolve} from 'node:path';
import {createRequire} from 'node:module';
import {fileURLToPath} from 'node:url';
import {hash,packageRoot,packageDigest} from './catalog.mjs';
import {byteSpan,resolveJsxDeclaration} from './compiler-jsx-resolution-v1.mjs';

const [outArg]=process.argv.slice(2),out=resolve(outArg);assert(!existsSync(out));mkdirSync(out);
const root=resolve('rust/target/real-package-hydration-native-v1/application');
const tsRoot=packageRoot(root,'typescript'),solid=packageRoot(root,'solid-js'),web=packageRoot(root,'@solidjs/web');
const require=createRequire(join(tsRoot,'package.json')),ts=require(join(tsRoot,'lib/typescript.js'));
const sourcePath=fileURLToPath(new URL('./compiler-jsx-resolution-cases-v1.tsx',import.meta.url));
const openPath=fileURLToPath(new URL('./compiler-jsx-resolution-open-v1.tsx',import.meta.url));
const compilerOptions={noEmit:true,strict:true,skipLibCheck:true,jsx:'preserve',jsxImportSource:'@solidjs/web',module:'ESNext',moduleResolution:'Bundler',target:'ES2022',types:[],baseUrl:root,
  paths:{'solid-js':[join(solid,'types/index.d.ts')],'@solidjs/web':[join(web,'types/index.d.ts')],'@solidjs/web/jsx-runtime':[join(web,'types/jsx.d.ts')],'@solidjs/web/jsx-dev-runtime':[join(web,'types/jsx.d.ts')]}};
const configPath=join(out,'tsconfig.json');writeFileSync(configPath,JSON.stringify({compilerOptions,files:[sourcePath]},null,2)+'\n');
const typing=spawnSync(process.execPath,[join(tsRoot,'lib/tsc.js'),'--noEmit','-p',configPath],{encoding:'utf8',timeout:60000});
writeFileSync(join(out,'typing.json'),JSON.stringify({exitCode:typing.status,stdout:typing.stdout,stderr:typing.stderr},null,2)+'\n');assert.equal(typing.status,0,typing.stdout+typing.stderr);
const config=ts.readConfigFile(configPath,ts.sys.readFile),parsed=ts.parseJsonConfigFileContent(config.config,ts.sys,out);
const program=ts.createProgram({rootNames:[sourcePath,openPath],options:parsed.options}),checker=program.getTypeChecker(),source=program.getSourceFile(sourcePath);
const openings=[];function visit(node){if(ts.isJsxOpeningElement(node)||ts.isJsxSelfClosingElement(node))openings.push(node);ts.forEachChild(node,visit);}visit(source);
const rows=[];
function check(tag,checked,kind){
  const opening=openings.find(node=>node.tagName.getText(source)===tag);assert(opening,tag);
  const value=resolveJsxDeclaration(ts,checker,opening,{checkedApplication:checked});assert.equal(value.status,'exact',tag);assert.equal(value.kind,kind,tag);
  if(value.declaration)assert(value.declarations.includes(value.declaration));
  const span=byteSpan(source,opening.tagName);assert.equal(Buffer.from(source.text).subarray(span.start,span.end).toString(),tag);
  rows.push({tag,checked,kind,declarations:value.declarations.length});return {opening,value};
}
const each=check('Each',true,'checked-overload-signature');
check('Solid.For',true,'checked-overload-signature');check('Show',true,'checked-overload-signature');
check('Each',false,'declared-overload-family');check('Show',false,'declared-overload-family');
check('For',true,'single-declaration');check('Registry.Local',true,'single-declaration');
assert.notEqual(resolveJsxDeclaration(ts,checker,openings.find(node=>node.tagName.getText(source)==='For'),{checkedApplication:true}).declaration.getSourceFile().fileName,each.value.declaration.getSourceFile().fileName);
const openSource=program.getSourceFile(openPath),missing=openSource.statements[0].body.statements[0].expression;
assert.equal(resolveJsxDeclaration(ts,checker,missing,{checkedApplication:true}).status,'open');
const foreignSignature={getSymbolAtLocation:node=>checker.getSymbolAtLocation(node),getAliasedSymbol:symbol=>checker.getAliasedSymbol(symbol),getResolvedSignature:()=>({declaration:source.statements.find(ts.isFunctionDeclaration)})};
assert.equal(resolveJsxDeclaration(ts,foreignSignature,each.opening,{checkedApplication:true}).status,'open');
const merged=ts.createSourceFile('/merged.d.ts','declare function M():any; declare namespace M { const x:number; }',ts.ScriptTarget.Latest,true);
const mergedSymbol={flags:0,declarations:merged.statements,getName:()=> 'M'};
assert.equal(resolveJsxDeclaration(ts,{getSymbolAtLocation:()=>mergedSymbol},each.opening,{checkedApplication:false}).status,'open');
const diagnostics=program.getSemanticDiagnostics(openSource);assert(diagnostics.some(row=>row.code===2304));
const result={authority:false,certification:false,passed:11,typing:{exitCode:0,publishedTypes:true},cases:rows,negativeCases:['unresolved-tag','foreign-signature-declaration','merged-function-namespace'],
  inputs:[sourcePath,openPath,fileURLToPath(import.meta.url),fileURLToPath(new URL('./compiler-jsx-resolution-v1.mjs',import.meta.url))].map(path=>({path,sha256:hash(readFileSync(path))})),
  packages:[tsRoot,solid,web].map(root=>({root,digest:packageDigest(root)})),limits:['These tests check exact declaration evidence, not runtime providers or package callback timing.','The unresolved negative case has TS2304 and produces no checker diagnostic.']};
writeFileSync(join(out,'results.json'),JSON.stringify(result,null,2)+'\n');console.log(JSON.stringify({passed:result.passed,typingExitCode:typing.status}));
