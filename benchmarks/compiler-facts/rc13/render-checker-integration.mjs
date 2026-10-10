// Render the saved development overlay for a restored candidate location.
// This prints a patch; it never changes the compiler or checker checkout.
import assert from 'node:assert/strict';
import {readFileSync,realpathSync} from 'node:fs';
import {resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
import {execFileSync} from 'node:child_process';

assert(process.argv.length===3,'Usage: node render-checker-integration.mjs <compiler-checkout>');
const compiler=realpathSync(resolve(process.argv[2]));
assert.equal(execFileSync('git',['rev-parse','HEAD'],{cwd:compiler,encoding:'utf8'}).trim(),
  '3ad4bbec37ae30f325a803cdb4271a71c86a2a2d','Restore the exact candidate distribution commit');
assert.equal(execFileSync('git',['status','--porcelain','--untracked-files=no'],{cwd:compiler,encoding:'utf8'}).trim(),
  '','Compiler tracked sources have changed');
const patch=readFileSync(new URL('./checker-integration.patch',import.meta.url),'utf8');
assert(patch.includes('@COMPILER_REPOSITORY_URI@'),'Integration patch has no local repository parameter');
const uri=pathToFileURL(compiler).href;
assert(!/[\r\n"\\]/.test(uri),'Unsupported local Git repository URI');
process.stdout.write(patch.replaceAll('@COMPILER_REPOSITORY_URI@',uri));
