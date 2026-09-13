# Self-hosted macOS runner

Falcon's macOS test coverage runs on a self-hosted runner rather than a
GitHub-hosted `macos-*` image. This directory holds the pieces of that setup
that are worth versioning; nothing here is secret.

## Why manual dispatch only

[`.github/workflows/test-macos-self-hosted.yml`](../../.github/workflows/test-macos-self-hosted.yml)
triggers on `workflow_dispatch` and nothing else. A self-hosted runner executes
repository code on a developer's own machine, so a pull request from an
untrusted contributor must not be able to run arbitrary code there just by
being opened. `ci.yml` stays on GitHub-hosted Ubuntu for its automatic
push/pull_request jobs for the same reason.

## Registering a runner

Each runner registration is scoped to a single repository. A runner registered
to another repo cannot serve this one, so this repo needs its own.

1. Open `Settings → Actions → Runners → New self-hosted runner` and pick your
   platform. GitHub fills in a registration token for you.
2. Run the commands it gives you in a **dedicated** directory, e.g.
   `~/actions-runner-falcon`, so it does not collide with a runner registered
   to a different repository.
3. Install it as a service so it survives reboots:

   ```bash
   ./svc.sh install
   ./svc.sh start
   ```

4. Confirm GitHub sees it:

   ```bash
   gh api repos/viveky259259/falcon/actions/runners
   ```

Copy `.env.example` to `.env` and set `RUNNER_ROOT` / `SVC_LABEL` to match
where you installed it. `.env` is gitignored.

## `svc.sh`

`svc.sh` here is the stock GitHub script with one fix. Stock `start()` calls
the deprecated `launchctl load -w` unconditionally; macOS refuses to load an
already-loaded job and prints

```
Load failed: 5: Input/output error
```

which looks like a failed start but is really a refused duplicate load of a
healthy service. The patched version checks whether the job is already
bootstrapped and uses `launchctl kickstart` in that case, and waits for the
job to settle before printing status. `stop` is symmetric: it reports
`is not loaded` instead of erroring.

Copy it over the installed one to apply:

```bash
cp scripts/runner/svc.sh "$RUNNER_ROOT/svc.sh"
```

**The runner auto-updates itself and regenerates `svc.sh` from
`bin/darwin.svc.sh.template`.** If the `Load failed` message reappears, an
update reverted the patch — copy this file over it again.

## Troubleshooting

**Runner shows `offline` but the process is running.** The GitHub API's status
field lags and flaps. Check the runner's own log instead:

```bash
tail -20 "$RUNNER_ROOT/_diag/$(ls -t "$RUNNER_ROOT/_diag" | grep '^Runner_' | head -1)"
```

`Listening for Jobs` means it is connected regardless of what the API says.

**A job stays queued while the runner reports `busy: true`.** If a session
drops while completing a job (`SocketException`, or a broker
`InternalServerError`), GitHub can leave the job claimed by that dead session.
The replacement session then fails to take it over:

```
POST .../acquirejob failed. HTTP Status: Conflict
Job message already acquired '...'. job assignment is invalid: MissingKey
```

Restarting the service does not help, because the stale claim lives on
GitHub's side. Cancel the wedged run and dispatch a fresh one:

```bash
gh run cancel <run-id> --repo viveky259259/falcon
gh workflow run test-macos-self-hosted.yml --repo viveky259259/falcon
```

**Force a clean restart** (works even when the job is already loaded):

```bash
launchctl kickstart -k "gui/$(id -u)/$SVC_LABEL"
```
