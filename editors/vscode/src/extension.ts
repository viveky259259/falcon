import * as path from "path";
import * as vscode from "vscode";
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  TransportKind,
} from "vscode-languageclient/node";
import { registerFalconChatParticipant } from "./chatParticipant";
import { buildHandleDiagnosticsMiddleware } from "./diagnosticsMiddleware";
import { ScoreLensProvider } from "./scoreLens";

let client: LanguageClient | undefined;
let statusBarItem: vscode.StatusBarItem;
let outputChannel: vscode.OutputChannel;

export function activate(context: vscode.ExtensionContext) {
  outputChannel = vscode.window.createOutputChannel("Falcon");

  const config = vscode.workspace.getConfiguration("falcon");
  if (!config.get<boolean>("enable", true)) {
    outputChannel.appendLine("Falcon is disabled in settings.");
    return;
  }

  statusBarItem = vscode.window.createStatusBarItem(
    vscode.StatusBarAlignment.Left,
    -100
  );
  statusBarItem.command = "falcon.showOutput";
  context.subscriptions.push(statusBarItem);

  startServer(context);
  registerFalconChatParticipant(context, outputChannel);

  const scoreLensProvider = new ScoreLensProvider(context, outputChannel);
  context.subscriptions.push(
    vscode.languages.registerCodeLensProvider(
      { scheme: "file", language: "dart" },
      scoreLensProvider
    )
  );

  context.subscriptions.push(
    vscode.commands.registerCommand("falcon.analyzeWorkspace", () => {
      if (client) {
        client.sendRequest("workspace/executeCommand", {
          command: "falcon.analyze",
        });
        vscode.window.showInformationMessage("Falcon: Re-analyzing workspace...");
      }
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand("falcon.fixAll", () => {
      if (client) {
        client.sendRequest("workspace/executeCommand", {
          command: "falcon.fixAll",
        });
      }
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand("falcon.restart", async () => {
      if (client) {
        await client.stop();
      }
      startServer(context);
      vscode.window.showInformationMessage("Falcon: Language server restarted.");
    })
  );

  context.subscriptions.push(
    vscode.commands.registerCommand("falcon.showOutput", () => {
      outputChannel.show();
    })
  );

  if (config.get<boolean>("autoFixOnSave", false)) {
    context.subscriptions.push(
      vscode.workspace.onWillSaveTextDocument((event) => {
        if (event.document.languageId !== "dart") return;
        event.waitUntil(applyAutoFixes(event.document));
      })
    );
  }

  context.subscriptions.push(
    vscode.workspace.onDidChangeConfiguration((e) => {
      if (e.affectsConfiguration("falcon")) {
        scoreLensProvider.refresh();
        if (client) {
          client.sendNotification("workspace/didChangeConfiguration", {
            settings: { falcon: vscode.workspace.getConfiguration("falcon") },
          });
        }
      }
    })
  );

  context.subscriptions.push(
    vscode.workspace.onDidSaveTextDocument((document) => {
      if (document.languageId === "dart") {
        scoreLensProvider.refresh();
      }
    })
  );
}

function startServer(context: vscode.ExtensionContext) {
  const config = vscode.workspace.getConfiguration("falcon");
  let serverPath = config.get<string>("executablePath", "");

  if (!serverPath) {
    serverPath = "falcon-lsp";
  }

  const serverOptions: ServerOptions = {
    run: { command: serverPath, transport: TransportKind.stdio },
    debug: { command: serverPath, transport: TransportKind.stdio },
  };

  const clientOptions: LanguageClientOptions = {
    documentSelector: [{ scheme: "file", language: "dart" }],
    synchronize: {
      fileEvents: vscode.workspace.createFileSystemWatcher("**/*.dart"),
    },
    outputChannel,
    traceOutputChannel: outputChannel,
    initializationOptions: {
      settings: vscode.workspace.getConfiguration("falcon"),
    },
    middleware: {
      // Stamp every Falcon diagnostic with `source: "falcon"` and a
      // namespaced `code: "falcon/<rule-id>"`, then drop diagnostics on
      // lines already claimed by the Dart analyzer (Dart-Code) so the two
      // extensions coexist without duplicate squiggles. See
      // `diagnosticsMiddleware.ts` for the full contract.
      handleDiagnostics: buildHandleDiagnosticsMiddleware(),
    },
  };

  client = new LanguageClient(
    "falcon",
    "Falcon Language Server",
    serverOptions,
    clientOptions
  );

  client.onDidChangeState((event) => {
    if (event.newState === 1) {
      outputChannel.appendLine("Falcon LSP server starting...");
    } else if (event.newState === 2) {
      outputChannel.appendLine("Falcon LSP server running.");
      updateStatusBar();
    } else if (event.newState === 3) {
      outputChannel.appendLine("Falcon LSP server stopped.");
      statusBarItem.hide();
    }
  });

  client.start().then(() => {
    setupDiagnosticsListener();
  });

  context.subscriptions.push({
    dispose: () => {
      if (client) {
        client.stop();
      }
    },
  });
}

function setupDiagnosticsListener() {
  vscode.languages.onDidChangeDiagnostics(() => {
    updateStatusBar();
  });
}

function updateStatusBar() {
  const config = vscode.workspace.getConfiguration("falcon");
  if (!config.get<boolean>("showStatusBar", true)) {
    statusBarItem.hide();
    return;
  }

  let errors = 0;
  let warnings = 0;
  let infos = 0;

  for (const [_uri, diagnostics] of vscode.languages.getDiagnostics()) {
    for (const d of diagnostics) {
      if (d.source !== "falcon") continue;
      switch (d.severity) {
        case vscode.DiagnosticSeverity.Error:
          errors++;
          break;
        case vscode.DiagnosticSeverity.Warning:
          warnings++;
          break;
        case vscode.DiagnosticSeverity.Information:
        case vscode.DiagnosticSeverity.Hint:
          infos++;
          break;
      }
    }
  }

  const total = errors + warnings + infos;

  if (total === 0) {
    statusBarItem.text = "$(check) Falcon";
    statusBarItem.tooltip = "Falcon: No issues found";
    statusBarItem.backgroundColor = undefined;
  } else {
    const parts: string[] = [];
    if (errors > 0) parts.push(`$(error) ${errors}`);
    if (warnings > 0) parts.push(`$(warning) ${warnings}`);
    if (infos > 0) parts.push(`$(info) ${infos}`);

    statusBarItem.text = `Falcon: ${parts.join(" ")}`;
    statusBarItem.tooltip = `Falcon: ${errors} errors, ${warnings} warnings, ${infos} info`;

    if (errors > 0) {
      statusBarItem.backgroundColor = new vscode.ThemeColor(
        "statusBarItem.errorBackground"
      );
    } else if (warnings > 0) {
      statusBarItem.backgroundColor = new vscode.ThemeColor(
        "statusBarItem.warningBackground"
      );
    } else {
      statusBarItem.backgroundColor = undefined;
    }
  }

  statusBarItem.show();
}

async function applyAutoFixes(
  document: vscode.TextDocument
): Promise<vscode.TextEdit[]> {
  const diagnostics = vscode.languages.getDiagnostics(document.uri);
  const falconDiags = diagnostics.filter((d) => d.source === "falcon");

  if (falconDiags.length === 0) return [];

  const codeActions = await vscode.commands.executeCommand<vscode.CodeAction[]>(
    "vscode.executeCodeActionProvider",
    document.uri,
    new vscode.Range(0, 0, document.lineCount, 0),
    vscode.CodeActionKind.QuickFix.value
  );

  if (!codeActions) return [];

  const edits: vscode.TextEdit[] = [];
  for (const action of codeActions) {
    if (
      action.isPreferred &&
      action.edit
    ) {
      const workspaceEdit = action.edit;
      const fileEdits = workspaceEdit.get(document.uri);
      if (fileEdits) {
        edits.push(...fileEdits);
      }
    }
  }

  return edits;
}

export function deactivate(): Thenable<void> | undefined {
  if (!client) {
    return undefined;
  }
  return client.stop();
}
