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
exports.ScoreLensProvider = void 0;
exports.shouldShowScoreLens = shouldShowScoreLens;
exports.scoreSnapshot = scoreSnapshot;
exports.scoreLensTitle = scoreLensTitle;
const cp = __importStar(require("child_process"));
const path = __importStar(require("path"));
const vscode = __importStar(require("vscode"));
const HISTORY_PREFIX = "falcon.scoreLens.history.";
const DEBOUNCE_MS = 250;
class ScoreCodeLens extends vscode.CodeLens {
    constructor(range, uri) {
        super(range);
        this.uri = uri;
    }
}
class ScoreLensProvider {
    constructor(context, outputChannel, runner = runFalconScore) {
        this.context = context;
        this.outputChannel = outputChannel;
        this.runner = runner;
        this.onDidChangeEmitter = new vscode.EventEmitter();
        this.onDidChangeCodeLenses = this.onDidChangeEmitter.event;
        this.pending = new Map();
        this.memory = new Map();
    }
    refresh() {
        this.onDidChangeEmitter.fire();
    }
    provideCodeLenses(document) {
        if (!shouldShowScoreLens(document)) {
            return [];
        }
        return [new ScoreCodeLens(new vscode.Range(0, 0, 0, 0), document.uri)];
    }
    async resolveCodeLens(codeLens) {
        if (!(codeLens instanceof ScoreCodeLens)) {
            codeLens.command = disabledCommand("Falcon score unavailable");
            return codeLens;
        }
        const document = await vscode.workspace.openTextDocument(codeLens.uri);
        if (!document || !shouldShowScoreLens(document)) {
            codeLens.command = disabledCommand("Falcon score unavailable");
            return codeLens;
        }
        const key = document.uri.toString();
        let pending = this.pending.get(key);
        if (!pending) {
            pending = this.resolveForDocument(codeLens, document).finally(() => {
                this.pending.delete(key);
            });
            this.pending.set(key, pending);
        }
        return pending;
    }
    async resolveForDocument(codeLens, document) {
        await delay(DEBOUNCE_MS);
        try {
            const previous = this.snapshotFor(document);
            const score = await this.runner(document);
            const current = scoreSnapshot(score);
            await this.remember(document, current);
            const delta = previous === undefined ? undefined : current.overall - previous.overall;
            const title = scoreLensTitle(current.overall, delta);
            codeLens.command = {
                title,
                command: "falcon.showOutput",
                tooltip: scoreLensTooltip(current, previous),
            };
        }
        catch (error) {
            const message = error instanceof Error ? error.message : String(error);
            this.outputChannel.appendLine(`Falcon score lens failed: ${message}`);
            codeLens.command = disabledCommand("Falcon score unavailable");
        }
        return codeLens;
    }
    snapshotFor(document) {
        const key = historyKey(document);
        return (this.memory.get(key) ??
            this.context.workspaceState.get(key));
    }
    async remember(document, snapshot) {
        const key = historyKey(document);
        this.memory.set(key, snapshot);
        await this.context.workspaceState.update(key, snapshot);
    }
}
exports.ScoreLensProvider = ScoreLensProvider;
function shouldShowScoreLens(document) {
    const config = vscode.workspace.getConfiguration("falcon");
    return (document.languageId === "dart" &&
        config.get("enable", true) &&
        config.get("scoreLens.enabled", true) &&
        !config.get("quietMode", false));
}
function scoreSnapshot(score) {
    return {
        overall: score.overall,
        findings: collectFindings(score),
        recordedAt: Date.now(),
    };
}
function scoreLensTitle(score, delta) {
    if (delta === undefined) {
        return `Falcon score ${score}`;
    }
    if (delta === 0) {
        return `Falcon score ${score} (no change)`;
    }
    const sign = delta > 0 ? "+" : "";
    return `Falcon score ${score} (${sign}${delta})`;
}
function scoreLensTooltip(current, previous) {
    const lines = [`Current Falcon score: ${current.overall}`];
    if (previous) {
        const delta = current.overall - previous.overall;
        const sign = delta > 0 ? "+" : "";
        lines.push(`Delta from previous save: ${sign}${delta}`);
    }
    if (current.findings.length > 0) {
        lines.push("", "Contributors:");
        lines.push(...current.findings.slice(0, 6).map((finding) => `- ${finding}`));
    }
    return lines.join("\n");
}
function disabledCommand(title) {
    return {
        title,
        command: "falcon.showOutput",
    };
}
function collectFindings(score) {
    const dimensions = [
        ["Resource safety", score.resource_safety],
        ["Error handling", score.error_handling],
        ["Type safety", score.type_safety],
        ["Security", score.security],
        ["Convention match", score.convention_match],
        ["Complexity", score.complexity],
    ];
    return dimensions.flatMap(([name, dimension]) => (dimension?.findings ?? []).map((finding) => `${name}: ${finding}`));
}
function runFalconScore(document) {
    const workspaceFolder = vscode.workspace.getWorkspaceFolder(document.uri);
    const cwd = workspaceFolder?.uri.fsPath ?? path.dirname(document.uri.fsPath);
    const target = workspaceFolder?.uri.fsPath ?? document.uri.fsPath;
    const falcon = resolveFalconCliPath();
    return new Promise((resolve, reject) => {
        cp.execFile(falcon, ["score", target, "--format", "json"], { cwd, timeout: 15000 }, (error, stdout, stderr) => {
            if (error) {
                reject(new Error(stderr.trim() || error.message));
                return;
            }
            try {
                resolve(JSON.parse(stdout));
            }
            catch (parseError) {
                reject(parseError);
            }
        });
    });
}
function resolveFalconCliPath() {
    const configured = vscode.workspace
        .getConfiguration("falcon")
        .get("executablePath", "");
    if (!configured) {
        return "falcon";
    }
    const base = path.basename(configured);
    if (base === "falcon-lsp" || base === "falcon-lsp.exe") {
        return path.join(path.dirname(configured), process.platform === "win32" ? "falcon.exe" : "falcon");
    }
    return configured;
}
function historyKey(document) {
    return `${HISTORY_PREFIX}${document.uri.toString()}`;
}
function delay(ms) {
    return new Promise((resolve) => setTimeout(resolve, ms));
}
//# sourceMappingURL=scoreLens.js.map