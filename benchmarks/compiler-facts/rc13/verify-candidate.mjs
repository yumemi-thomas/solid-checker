import assert from 'node:assert/strict';
import {readFileSync,writeFileSync} from 'node:fs';
import {resolve,join} from 'node:path';
import {createHash} from 'node:crypto';
import {execFileSync} from 'node:child_process';
import {compareRows} from './compare.mjs';
import {compilerSourceManifest} from '../../../scripts/check-compiler-facts-identity.mjs';

const candidate=resolve('rust/target/compiler-rc13-rebase');
const root=resolve('benchmarks/compiler-facts/rc13');
const identity={upstreamRevision:'5efaf260becb32293f2bcb4d32f8be72be6de674',
  implementationRevision:'c04c48779812d3d87166da3741c625748458c62f',
  distributionRevision:'3ad4bbec37ae30f325a803cdb4271a71c86a2a2d',
  semanticTraceVersion:3,compilerFactsProtocol:2};
const git=(...args)=>execFileSync('git',args,{cwd:candidate,encoding:'utf8'}).trim();
const hash=path=>createHash('sha256').update(readFileSync(path)).digest('hex');
const json=path=>JSON.parse(readFileSync(path,'utf8'));
assert.equal(git('rev-parse','HEAD'),identity.distributionRevision);
assert.equal(git('rev-parse','HEAD^'),identity.implementationRevision);
assert.equal(git('diff','--name-only','HEAD^','HEAD'),'packages/compiler/src/semantic_trace.rs');
assert.equal(git('diff','--name-only',identity.upstreamRevision,'HEAD','--',':!packages/compiler'),'');
execFileSync('git',['merge-base','--is-ancestor',identity.upstreamRevision,identity.implementationRevision],{cwd:candidate});
const file='packages/compiler/src/semantic_trace.rs';
const before=git('show',`${identity.implementationRevision}:${file}`);
const after=git('show',`${identity.distributionRevision}:${file}`);
const old='pub const SEMANTIC_TRACE_IMPLEMENTATION_REVISION: &str = "c58f531a7707529273a23f343a087036e493f12d";';
const replacement=`pub const SEMANTIC_TRACE_IMPLEMENTATION_REVISION: &str = "${identity.implementationRevision}";`;
assert(before.includes(old));assert.equal(before.replace(old,replacement),after);
const patch=execFileSync('git',['diff','--binary',identity.upstreamRevision,'HEAD','--','packages/compiler'],{cwd:candidate,maxBuffer:8*1024*1024});
assert.deepEqual(patch,readFileSync(join(root,'semantic-facts.patch')));
execFileSync('git',['bundle','verify',join(root,'candidate.bundle')],{cwd:resolve('rust/target/compiler-rc13-upstream'),stdio:'pipe'});
const comparison=resolve('rust/target/compiler-rc13-comparison-final-v2');
const requests=json(join(comparison,'requests.json'));
const upstream=json(join(comparison,'upstream.json')),fork=json(join(comparison,'fork.json'));
const rows=compareRows(requests,upstream,fork);
assert.equal(rows.length,3285);
const installed=json(resolve('rust/target/compiler-rc13-installed-comparison-final/results.json'));
assert.equal(installed.summary.outputAndDiagnosticMatches,rows.length);
const baseline=join(candidate,'packages/compiler/tests/transform-output-baseline.txt');
assert.equal(hash(baseline),hash(join(comparison,'upstream-transform-output-baseline.txt')));
const report=json(join(comparison,'results.json'));
for(const binary of report.binaries)assert.equal(hash(binary.path),binary.sha256);
const checks={};
for(const [name,path,pattern] of [
  ['rustCore','compiler-rc13-no-default-v3.log',/6 passed; 0 failed; 1 ignored/],
  ['rustDefault','compiler-rc13-default-v2.log',/63 passed; 0 failed/],
  ['rustTsrx','compiler-rc13-tsrx.log',/53 passed; 0 failed/],
  ['adapter','compiler-rc13-adapter.log',/13 passed; 0 failed/],
  ['javascript','compiler-rc13-js-final.log',/5938 passed/],
  ['candidateClippy','compiler-rc13-clippy-scoped.log',/Finished/],
  ['checkerUniversalFmtClippy','compiler-rc13-checker-verify-fast.log',/Finished/],
]){
  const absolute=resolve('rust/target',path),text=readFileSync(absolute,'utf8');
  assert(pattern.test(text),`${name} result missing`);assert(!/error:|test result: FAILED|Tests .*failed/.test(text),`${name} failed`);
  checks[name]={path:absolute,sha256:hash(absolute)};
}
const inputs=['compare.mjs','compare.test.mjs','compare-installed.mjs','capture-release.mjs','verify-candidate.mjs',
  'probe/Cargo.toml','probe/Cargo.lock','probe/src/main.rs','probe-upstream/Cargo.toml','probe-upstream/Cargo.lock',
  'adapter/Cargo.toml','adapter/Cargo.lock','adapter/src/lib.rs','semantic-facts.patch','candidate.bundle']
  .map(path=>({path,sha256:hash(join(root,path))}));
const result={authority:false,certification:false,completedAt:new Date().toISOString(),version:'2.0.0-rc.13',
  identity,sourceManifestSha256:compilerSourceManifest(identity,'solid-v2'),
  sourceComparison:{summary:report.summary,path:comparison,resultsSha256:hash(join(comparison,'results.json')),
    requestsSha256:hash(join(comparison,'requests.json')),upstreamSha256:hash(join(comparison,'upstream.json')),forkSha256:hash(join(comparison,'fork.json'))},
  publishedComparison:{summary:installed.summary,native:installed.native,
    resultsSha256:hash(resolve('rust/target/compiler-rc13-installed-comparison-final/results.json'))},
  release:json(resolve('rust/target/compiler-rc13-release-v1/release.json')),checks,inputs,
  productionPinChanged:false,limits:['No production checker pin migration or full verification.','Universal/Dynamic, authored TSRX and bypassed files refuse facts.','Generated operation enumeration remains partial.','Published native comparison covers darwin-arm64 only.','Output equality does not establish runtime package compatibility or warning precision.']};
writeFileSync(join(root,'candidate-evidence.json'),JSON.stringify(result,null,2)+'\n');
console.log(JSON.stringify({identity,comparisons:rows.length,checks:Object.keys(checks)}));
