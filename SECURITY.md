# Security Policy

## Reporting a vulnerability

Please report security issues privately through GitHub:
**[Report a vulnerability](https://github.com/viveky259259/falcon/security/advisories/new)**
(the repository's Security tab, then "Report a vulnerability").

Do not open a public issue, pull request, or discussion for a suspected
vulnerability.

Include what you can of:

- the Falcon version (`falcon --version`) and platform,
- the command, endpoint, or integration involved (CLI, `falcon x api`,
  `falcon-mcp`, `falcon-lsp`, the GitHub Action, or the GitHub App),
- steps or a minimal project that reproduces it, and the impact you expect.

## What to expect

Falcon is maintained by a small team. We aim to acknowledge reports within
7 days and to agree on a fix and disclosure timeline with you after that.
We will credit you in the release notes unless you ask us not to.

## Supported versions

Security fixes land on the latest release only. Upgrade with
`falcon x update` or re-run the install script.

## Scope

In scope: the `falcon`, `falcon-lsp`, and `falcon-mcp` binaries, the HTTP
bridge (`falcon x api`), the install script, the npm wrapper, the GitHub
Action in `action/`, and the GitHub App in `github-app/`.

Out of scope: findings in third-party dependencies with no demonstrated
impact on Falcon, and reports that need an attacker who can already write
to your project's files or run commands as you.
