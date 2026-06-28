"use strict";
var __createBinding = (this && this.__createBinding) || (Object.create ? (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    var desc = Object.getOwnPropertyDescriptor(m, k);
    if (!desc || ("get" in desc ? !m.__esModule : desc.writable || desc.configurable)) {
      desc = { enumerable: true, get: function() { return m[k]; } };
    }
    Object.defineProperty(o, k2, desc);
}) : (function(o, m, k, k2) {
    if (k2 === undefined) k2 = k;
    o[k2] = m[k];
}));
var __setModuleDefault = (this && this.__setModuleDefault) || (Object.create ? (function(o, v) {
    Object.defineProperty(o, "default", { enumerable: true, value: v });
}) : function(o, v) {
    o["default"] = v;
});
var __importStar = (this && this.__importStar) || (function () {
    var ownKeys = function(o) {
        ownKeys = Object.getOwnPropertyNames || function (o) {
            var ar = [];
            for (var k in o) if (Object.prototype.hasOwnProperty.call(o, k)) ar[ar.length] = k;
            return ar;
        };
        return ownKeys(o);
    };
    return function (mod) {
        if (mod && mod.__esModule) return mod;
        var result = {};
        if (mod != null) for (var k = ownKeys(mod), i = 0; i < k.length; i++) if (k[i] !== "default") __createBinding(result, mod, k[i]);
        __setModuleDefault(result, mod);
        return result;
    };
})();
Object.defineProperty(exports, "__esModule", { value: true });
exports.activate = activate;
exports.deactivate = deactivate;
const vscode = __importStar(require("vscode"));
const node_1 = require("vscode-languageclient/node");
const diagnosticsMiddleware_1 = require("./diagnosticsMiddleware");
const scoreLens_1 = require("./scoreLens");
let client;
let statusBarItem;
let outputChannel;
function activate(context) {
    outputChannel = vscode.window.createOutputChannel("Falcon");
    const config = vscode.workspace.getConfiguration("falcon");
    if (!config.get("enable", true)) {
        outputChannel.appendLine("Falcon is disabled in settings.");
        return;
    }
    statusBarItem = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, -100);
    statusBarItem.command = "falcon.showOutput";
    context.subscriptions.push(statusBarItem);
    startServer(context);
    const scoreLensProvider = new scoreLens_1.ScoreLensProvider(context, outputChannel);
    context.subscriptions.push(vscode.languages.registerCodeLensProvider({ scheme: "file", language: "dart" }, scoreLensProvider));
    context.subscriptions.push(vscode.commands.registerCommand("falcon.analyzeWorkspace", () => {
        if (client) {
            client.sendRequest("workspace/executeCommand", {
                command: "falcon.analyze",
            });
            vscode.window.showInformationMessage("Falcon: Re-analyzing workspace...");
        }
    }));
    context.subscriptions.push(vscode.commands.registerCommand("falcon.fixAll", () => {
        if (client) {
            client.sendRequest("workspace/executeCommand", {
                command: "falcon.fixAll",
            });
        }
    }));
    context.subscriptions.push(vscode.commands.registerCommand("falcon.restart", async () => {
        if (client) {
            await client.stop();
        }
        startServer(context);
        vscode.window.showInformationMessage("Falcon: Language server restarted.");
    }));
    context.subscriptions.push(vscode.commands.registerCommand("falcon.showOutput", () => {
        outputChannel.show();
    }));
    if (config.get("autoFixOnSave", false)) {
        context.subscriptions.push(vscode.workspace.onWillSaveTextDocument((event) => {
            if (event.document.languageId !== "dart")
                return;
            event.waitUntil(applyAutoFixes(event.document));
        }));
    }
    context.subscriptions.push(vscode.workspace.onDidChangeConfiguration((e) => {
        if (e.affectsConfiguration("falcon")) {
            scoreLensProvider.refresh();
            if (client) {
                client.sendNotification("workspace/didChangeConfiguration", {
                    settings: { falcon: vscode.workspace.getConfiguration("falcon") },
                });
            }
        }
    }));
    context.subscriptions.push(vscode.workspace.onDidSaveTextDocument((document) => {
        if (document.languageId === "dart") {
            scoreLensProvider.refresh();
        }
    }));
}
function startServer(context) {
    const config = vscode.workspace.getConfiguration("falcon");
    let serverPath = config.get("executablePath", "");
    if (!serverPath) {
        serverPath = "falcon-lsp";
    }
    const serverOptions = {
        run: { command: serverPath, transport: node_1.TransportKind.stdio },
        debug: { command: serverPath, transport: node_1.TransportKind.stdio },
    };
    const clientOptions = {
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
            handleDiagnostics: (0, diagnosticsMiddleware_1.buildHandleDiagnosticsMiddleware)(),
        },
    };
    client = new node_1.LanguageClient("falcon", "Falcon Language Server", serverOptions, clientOptions);
    client.onDidChangeState((event) => {
        if (event.newState === 1) {
            outputChannel.appendLine("Falcon LSP server starting...");
        }
        else if (event.newState === 2) {
            outputChannel.appendLine("Falcon LSP server running.");
            updateStatusBar();
        }
        else if (event.newState === 3) {
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
    if (!config.get("showStatusBar", true)) {
        statusBarItem.hide();
        return;
    }
    let errors = 0;
    let warnings = 0;
    let infos = 0;
    for (const [_uri, diagnostics] of vscode.languages.getDiagnostics()) {
        for (const d of diagnostics) {
            if (d.source !== "falcon")
                continue;
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
    }
    else {
        const parts = [];
        if (errors > 0)
            parts.push(`$(error) ${errors}`);
        if (warnings > 0)
            parts.push(`$(warning) ${warnings}`);
        if (infos > 0)
            parts.push(`$(info) ${infos}`);
        statusBarItem.text = `Falcon: ${parts.join(" ")}`;
        statusBarItem.tooltip = `Falcon: ${errors} errors, ${warnings} warnings, ${infos} info`;
        if (errors > 0) {
            statusBarItem.backgroundColor = new vscode.ThemeColor("statusBarItem.errorBackground");
        }
        else if (warnings > 0) {
            statusBarItem.backgroundColor = new vscode.ThemeColor("statusBarItem.warningBackground");
        }
        else {
            statusBarItem.backgroundColor = undefined;
        }
    }
    statusBarItem.show();
}
async function applyAutoFixes(document) {
    const diagnostics = vscode.languages.getDiagnostics(document.uri);
    const falconDiags = diagnostics.filter((d) => d.source === "falcon");
    if (falconDiags.length === 0)
        return [];
    const codeActions = await vscode.commands.executeCommand("vscode.executeCodeActionProvider", document.uri, new vscode.Range(0, 0, document.lineCount, 0), vscode.CodeActionKind.QuickFix.value);
    if (!codeActions)
        return [];
    const edits = [];
    for (const action of codeActions) {
        if (action.isPreferred &&
            action.edit) {
            const workspaceEdit = action.edit;
            const fileEdits = workspaceEdit.get(document.uri);
            if (fileEdits) {
                edits.push(...fileEdits);
            }
        }
    }
    return edits;
}
function deactivate() {
    if (!client) {
        return undefined;
    }
    return client.stop();
}
//# sourceMappingURL=extension.js.map