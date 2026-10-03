// Checks the new semantic cases against the audited published typings,
// independently of the reduced semantic fixture declarations.
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {resolve} from 'node:path';
import ts from '../../../packages/cli/node_modules/typescript/lib/typescript.js';

const install=resolve(process.argv[2]??'rust/target/tsc-oracle/v2');
const packageVersion=name=>JSON.parse(readFileSync(resolve(install,'node_modules',name,'package.json'))).version;
const versions=Object.fromEntries(['solid-js','@solidjs/signals','@solidjs/web'].map(name=>[name,packageVersion(name)]));
for(const version of Object.values(versions))assert.equal(version,'2.0.0-rc.9');
const options={strict:true,noEmit:true,skipLibCheck:true,jsx:ts.JsxEmit.Preserve,
  target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.ESNext,moduleResolution:ts.ModuleResolutionKind.Bundler,
  baseUrl:install,paths:{'solid-js':['node_modules/solid-js/types/index.d.ts'],
    '@solidjs/web/jsx-runtime':['node_modules/@solidjs/web/types/jsx.d.ts']},jsxImportSource:'@solidjs/web'};
const files=['Async.tsx','Feedback.ts','helpers.ts'].map(name=>resolve('fixtures/reactive-ir/callee-callback-timing',name));
const program=ts.createProgram(files,options);
assert(!program.getSourceFiles().some(source=>source.fileName.endsWith('/callee-callback-timing/solid-js.d.ts')));
const errors=ts.getPreEmitDiagnostics(program).filter(item=>item.category===ts.DiagnosticCategory.Error);
const result={typescript:ts.version,versions,files,errors:errors.map(item=>({code:item.code,message:ts.flattenDiagnosticMessageText(item.messageText,'\n')}))};
console.log(JSON.stringify(result,null,2));assert.equal(errors.length,0,'Published typings already report the semantic case');
