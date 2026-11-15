/**
 * Falcon GitHub App
 * Automatically analyzes Flutter PRs and posts quality reports.
 *
 * Start it with `npm start` (`probot run ./index.js`); see .env.example.
 */

const { execFile } = require("child_process");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");

const MARKER = "Falcon Analysis";
const BASE_REF = "refs/remotes/pr/base";
const HEAD_REF = "refs/remotes/pr/head";

/** Run a program without a shell; resolves with stdout. */
function run(executable, args, options) {
  return new Promise((resolve, reject) => {
    execFile(executable, args, { maxBuffer: 16 * 1024 * 1024, ...options }, (error, stdout) => {
      if (error) reject(error);
      else resolve(String(stdout));
    });
  });
}

/**
 * Git environment that authenticates with the installation token through
 * git's environment config, so the token never appears in argv or a URL.
 */
function gitEnv(token) {
  const basic = Buffer.from(`x-access-token:${token}`).toString("base64");
  return {
    ...process.env,
    GIT_TERMINAL_PROMPT: "0",
    GIT_CONFIG_COUNT: "1",
    GIT_CONFIG_KEY_0: "http.extraHeader",
    GIT_CONFIG_VALUE_0: `Authorization: basic ${basic}`,
  };
}

/**
 * Fetch the PR head and base branch from the base repository. GitHub keeps
 * every PR's commits at refs/pull/<n>/head, so this works for private repos
 * and for PRs from forks, including deleted forks. Full history lets
 * `falcon review` find the merge base. Contents are fetched up front because
 * later git calls run inside falcon, which never sees the token.
 */
async function checkoutPullRequest(dir, repo, pr, env) {
  const git = (args, timeout = 120000) => run("git", ["-C", dir, ...args], { env, timeout });
  await run("git", ["init", "-q", dir], { env, timeout: 30000 });
  await git(["remote", "add", "origin", repo.clone_url]);
  await git([
    "fetch", "--no-tags", "origin",
    `+refs/heads/${pr.base.ref}:${BASE_REF}`,
    `+refs/pull/${pr.number}/head:${HEAD_REF}`,
  ], 300000);
  await git(["checkout", "-q", "--detach", HEAD_REF], 300000);
  // The PR is untrusted. `falcon review` runs `dart analyze` (which can load
  // analyzer plugins from the project) whenever .dart_tool/package_config.json
  // exists, so never let the PR supply one.
  fs.rmSync(path.join(dir, ".dart_tool"), { recursive: true, force: true });
}

async function upsertComment(context, owner, repoName, number, body) {
  const existing = await context.octokit.issues.listComments({
    owner,
    repo: repoName,
    issue_number: number,
  });
  const previous = existing.data.find((c) => c.body && c.body.includes(MARKER));
  if (previous) {
    await context.octokit.issues.updateComment({
      owner,
      repo: repoName,
      comment_id: previous.id,
      body,
    });
  } else {
    await context.octokit.issues.createComment({
      owner,
      repo: repoName,
      issue_number: number,
      body,
    });
  }
}

module.exports = (app) => {
  app.log.info("Falcon GitHub App loaded");

  app.on("pull_request.opened", analyze);
  app.on("pull_request.synchronize", analyze);

  async function analyze(context) {
    const pr = context.payload.pull_request;
    const repo = context.payload.repository;
    const owner = repo.owner.login;
    const repoName = repo.name;

    app.log.info(`Analyzing PR #${pr.number} on ${owner}/${repoName}`);

    const cloneDir = fs.mkdtempSync(path.join(os.tmpdir(), "falcon-pr-"));
    try {
      const { token } = await context.octokit.auth({ type: "installation" });
      await checkoutPullRequest(cloneDir, repo, pr, gitEnv(token));

      let scoreJson;
      try {
        scoreJson = await run("falcon", ["score", cloneDir, "--format", "json"], {
          timeout: 120000,
        });
      } catch (e) {
        app.log.error(`falcon score failed: ${e.message}`);
        scoreJson = '{"overall": 0, "grade": "?", "total_issues": 0, "file_count": 0}';
      }

      let comment;
      try {
        comment = await run(
          "falcon",
          ["review", cloneDir, "--diff", BASE_REF, "--format", "gh"],
          { timeout: 120000 }
        );
      } catch (e) {
        app.log.error(`falcon review failed: ${e.message}`);
        const score = JSON.parse(scoreJson);
        comment = `## ${MARKER}\n\nScore: ${score.overall}/100 | Issues: ${score.total_issues}\n`;
      }

      await upsertComment(context, owner, repoName, pr.number, comment);
      app.log.info(`Posted Falcon analysis for PR #${pr.number}`);
    } catch (error) {
      app.log.error(`Falcon analysis failed: ${error.message}`);
    } finally {
      fs.rmSync(cloneDir, { recursive: true, force: true });
    }
  }
};
