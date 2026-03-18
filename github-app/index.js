/**
 * Falcon GitHub App
 * Automatically analyzes Flutter PRs and posts quality reports.
 */

const { execSync } = require("child_process");

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

    try {
      // Clone the PR branch
      const cloneDir = `/tmp/falcon-${owner}-${repoName}-${pr.number}`;
      execSync(`rm -rf ${cloneDir}`);
      execSync(
        `git clone --depth 1 --branch ${pr.head.ref} ${pr.head.repo.clone_url} ${cloneDir}`,
        { timeout: 60000 }
      );

      // Run Falcon analysis
      let scoreJson;
      try {
        scoreJson = execSync(`falcon ai-score ${cloneDir} --json`, {
          timeout: 120000,
        }).toString();
      } catch (e) {
        scoreJson = '{"overall": 0, "grade": "?", "total_issues": 0, "file_count": 0}';
      }

      // Generate PR comment
      let comment;
      try {
        comment = execSync(`falcon pr-comment ${cloneDir} --dry-run`, {
          timeout: 120000,
        }).toString();
      } catch (e) {
        const score = JSON.parse(scoreJson);
        comment = `## Falcon Analysis\n\nScore: ${score.overall}/100 | Issues: ${score.total_issues}\n`;
      }

      // Post comment
      const existingComments = await context.octokit.issues.listComments({
        owner,
        repo: repoName,
        issue_number: pr.number,
      });

      const falconComment = existingComments.data.find(
        (c) => c.body && c.body.includes("Falcon Analysis")
      );

      if (falconComment) {
        await context.octokit.issues.updateComment({
          owner,
          repo: repoName,
          comment_id: falconComment.id,
          body: comment,
        });
      } else {
        await context.octokit.issues.createComment({
          owner,
          repo: repoName,
          issue_number: pr.number,
          body: comment,
        });
      }

      // Cleanup
      execSync(`rm -rf ${cloneDir}`);

      app.log.info(`Posted Falcon analysis for PR #${pr.number}`);
    } catch (error) {
      app.log.error(`Falcon analysis failed: ${error.message}`);
    }
  }
};
