const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const {spawnSync} = require('node:child_process');
const temp = fs.mkdtempSync(path.join(os.tmpdir(), 'falcon-action-test-'));
try {
  const executable=path.join(temp,'falcon');
  fs.writeFileSync(executable, '#!/usr/bin/env node\nconsole.log(JSON.stringify(process.argv.slice(2)));\n',{mode:0o755});
  const baseline='baseline folder/report.json';
  const result=spawnSync('bash',[path.join(__dirname,'../run.sh')],{encoding:'utf8',env:{...process.env,PATH:temp+path.delimiter+process.env.PATH,INPUT_PATH:'project with spaces/$(echo injected)',INPUT_FORMAT:'json',INPUT_FAIL_ON:'error',INPUT_SINCE:'HEAD~1',INPUT_BASELINE:baseline,INPUT_CONFIG:'config folder/falcon.yaml'}});
  assert.equal(result.status,0,result.stderr);
  assert.deepEqual(JSON.parse(result.stdout),['check','project with spaces/$(echo injected)','--format','json','--fail-on','error','--since','HEAD~1','--baseline',baseline,'--config','config folder/falcon.yaml']);
  console.log('Action literal arguments and baseline filename regression passed');
} finally { fs.rmSync(temp,{recursive:true,force:true}); }
