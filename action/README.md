# Falcon Lint action

`baseline` takes a baseline JSON filename, matching `falcon check --baseline FILE`.
Project, config, baseline and Git ref values are passed as literal arguments.

```yaml
- uses: viveky259259/falcon/action@v0.5.3
  with:
    path: .
    format: json
    config: falcon.yaml
    baseline: falcon-baseline.json
```

Generate a baseline with `falcon check . --update-baseline falcon-baseline.json`.
Run the argument contract test locally with `node action/test/run.test.js`.
