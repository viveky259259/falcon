const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const path = require('node:path');

const TOKEN = 'ghs_installation_token_fixture';
const BRANCH = 'review-$(printf${IFS}injected)';

function loadApp() {
  const calls = [];
  const removed = [];
  const comments = [];
  let handler;
  const sandbox = {
    module: { exports: {} },
    Buffer,
    process: { env: { PATH: '/usr/bin' } },
    require(name) {
      if (name === 'child_process' || name === 'node:child_process') return {
        execSync() { assert.fail('Webhook must never invoke a shell'); },
        execFileSync() { assert.fail('Webhook must not block the event loop'); },
        execFile(executable, args, options, callback) {
          calls.push({ executable, args, options });
          const out = executable === 'falcon' && args[0] === 'score'
            ? '{"overall":100,"total_issues":0}'
            : '## Falcon Analysis\n';
          setImmediate(() => callback(null, out, ''));
        },
      };
      if (name === 'fs' || name === 'node:fs') return {
        mkdtempSync() { return '/fixture/unique'; },
        rmSync(p) { removed.push(p); },
      };
      if (name === 'os' || name === 'node:os') return { tmpdir() { return '/fixture'; } };
      if (name === 'path' || name === 'node:path') return path;
      throw Error('Unexpected dependency: ' + name);
    },
  };
  vm.runInNewContext(fs.readFileSync(path.join(__dirname, '../index.js'), 'utf8'), sandbox);
  sandbox.module.exports({
    log: { info() {}, error(message) { throw Error(message); } },
    on(event, cb) { handler = cb; },
  });
  const context = (headRepo) => ({
    payload: {
      pull_request: { number: 1, head: { ref: BRANCH, repo: headRepo }, base: { ref: 'main' } },
      repository: { owner: { login: 'owner' }, name: 'repo', clone_url: 'https://github.com/owner/repo.git' },
    },
    octokit: {
      async auth({ type }) { assert.equal(type, 'installation'); return { token: TOKEN }; },
      issues: {
        async listComments() { return { data: [] }; },
        async createComment(c) { comments.push(c); },
        async updateComment() { assert.fail('No existing comment to update'); },
      },
    },
  });
  return { calls, removed, comments, run: (headRepo) => handler(context(headRepo)) };
}

(async () => {
  // A PR from a fork, then one whose fork was deleted (head.repo is null).
  for (const headRepo of [{ clone_url: 'https://github.com/fork/repo.git' }, null]) {
    const { calls, removed, comments, run } = loadApp();
    await run(headRepo);

    for (const call of calls) {
      assert.equal(call.options.shell, undefined, 'No call may use a shell');
      const argv = call.args.join(' ');
      assert(!argv.includes(BRANCH), 'The untrusted branch name must never reach argv');
      assert(!argv.includes(TOKEN), 'The token must never appear in argv');
      if (call.executable === 'falcon') {
        assert(!JSON.stringify(call.options.env || {}).includes(TOKEN), 'falcon must not receive the token');
      }
    }

    const fetch = calls.find((c) => c.executable === 'git' && c.args.includes('fetch'));
    assert(fetch, 'The PR must be fetched with git');
    assert(fetch.args.includes('+refs/pull/1/head:refs/remotes/pr/head'), 'Fetch the PR ref from the base repo');
    assert(fetch.args.includes('+refs/heads/main:refs/remotes/pr/base'), 'Fetch the base branch');
    assert(!fetch.args.includes('--depth'), 'Shallow clones break falcon review');
    assert(fetch.options.env.GIT_CONFIG_VALUE_0.startsWith('Authorization: basic '), 'Authenticate via git env config');

    const remote = calls.find((c) => c.executable === 'git' && c.args.includes('remote'));
    assert(remote.args.includes('https://github.com/owner/repo.git'), 'Fetch from the base repository, not the fork');

    const review = calls.find((c) => c.executable === 'falcon' && c.args[0] === 'review');
    assert(review, 'falcon review must run');
    // Arrays from the vm sandbox have another realm's prototype, so compare text.
    assert.equal(review.args.slice(2, 4).join(' '), '--diff refs/remotes/pr/base', 'Review the whole PR against its base');

    assert(removed.includes('/fixture/unique/.dart_tool'), 'Strip .dart_tool so the PR cannot enable dart analyze');
    assert(removed.includes('/fixture/unique'), 'The working directory must be cleaned');
    assert.equal(comments.length, 1, 'One PR comment must be posted');
  }
  console.log('GitHub app execution regression passed');
})().catch((error) => { console.error(error); process.exitCode = 1; });
