const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const path = require('node:path');
const calls = [];
let callback;
let cleanup = 0;
const sandbox = {
  module: {exports: {}},
  require(name) {
    if (name === 'child_process' || name === 'node:child_process') return {
      execSync() { assert.fail('Webhook must never invoke a shell'); },
      execFileSync(executable, args, options) {
        calls.push({executable, args, options});
        return Buffer.from(executable === 'falcon' && args[0] === 'score' ? '{"overall":100}' : '## Falcon Analysis');
      }
    };
    if (name === 'fs' || name === 'node:fs') return {mkdtempSync(){return '/fixture/unique';},rmSync(){cleanup++;}};
    if (name === 'os' || name === 'node:os') return {tmpdir(){return '/fixture';}};
    if (name === 'path' || name === 'node:path') return path;
    throw Error('Unexpected dependency: ' + name);
  }
};
vm.runInNewContext(fs.readFileSync(path.join(__dirname,'../index.js'),'utf8'), sandbox);
sandbox.module.exports({log:{info(){},error(error){throw Error(error);}},on(event, cb){callback=cb;}});
(async () => {
  const branch = 'review-$(printf${IFS}injected)';
  await callback({payload:{pull_request:{number:1,head:{ref:branch,repo:{clone_url:'https://example.test/repo.git'}}},repository:{owner:{login:'owner'},name:'repo'}},octokit:{issues:{async listComments(){return {data:[]};},async createComment(){}}}});
  const clone=calls.find(call => call.executable==='git');
  assert(clone, 'Clone must use executable and arguments');
  assert(clone.args.includes(branch), 'Branch must remain one literal argument');
  assert.equal(clone.options.shell, undefined);
  assert.equal(cleanup,1,'Unique working directory must be cleaned');
  assert(calls.some(call=>call.executable==='falcon' && call.args[0]==='score'));
  console.log('GitHub app shell-free execution regression passed');
})().catch(error=>{console.error(error);process.exitCode=1;});
