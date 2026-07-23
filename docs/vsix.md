# Falcon VS Code Extension

Falcon's VS Code extension wraps `falcon-lsp` for diagnostics and uses the local
`falcon` CLI for score and rule explanations. The extension does not call an LLM
for its built-in chat responses.

For package and install smoke-test steps, see [VS Code Extension Install](vsix-install.md).

## Chat Participant

In VS Code Chat, type `@falcon` and ask one of:

```text
why did my score drop?
```

```text
explain rule avoid-empty-catch
```

The score response runs:

```bash
falcon score <workspace> --format json
```

Falcon stores the previous chat score in VS Code workspace state and compares it
with the new JSON result. The response lists the overall delta, changed score
dimensions, and current contributors from the local score payload.

The rule response runs:

```bash
falcon x explain <rule-id>
```

and returns the local rule explanation as text.

## Requirements

- `falcon-lsp` must be available on `PATH`, or configured with
  `falcon.executablePath`.
- `falcon` must be available on `PATH`, or next to the configured
  `falcon-lsp` binary.
- VS Code must provide the Chat Participant API.

## Screenshot

Release screenshot target: `docs/assets/vsix-chat.png`.

Before publishing a VSIX release, capture VS Code Chat showing `@falcon why did
my score drop?` against a Flutter workspace and save it at that path.
