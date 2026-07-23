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
exports.registerFalconChatParticipant = registerFalconChatParticipant;
const cp = __importStar(require("child_process"));
const path = __importStar(require("path"));
const vscode = __importStar(require("vscode"));
const CHAT_PARTICIPANT_ID = "falcon.chat";
const SCORE_HISTORY_KEY = "falcon.chat.scoreHistory.";
function registerFalconChatParticipant(context, outputChannel) {
    const chat = vscode.chat;
    if (!chat?.createChatParticipant) {
        outputChannel.appendLine("Falcon chat participant unavailable: VS Code Chat API is not present.");
        return undefined;
    }
    const participant = chat.createChatParticipant(CHAT_PARTICIPANT_ID, async (request, _context, response, token) => {
        try {
            const prompt = request.prompt.trim();
            if (isScoreDropIntent(prompt, request.command)) {
                await explainScoreDrop(context, response, token);
                return;
            }
            const rule = parseRuleIntent(prompt, request.command);
            if (rule) {
                await explainRule(rule, response, token);
                return;
            }
            response.markdown([
                "Ask Falcon one of:",
                "",
                "- `why did my score drop?`",
                "- `explain rule avoid-empty-catch`",
            ].join("\n"));
        }
        catch (error) {
            const message = error instanceof Error ? error.message : String(error);
            outputChannel.appendLine(`Falcon chat failed: ${message}`);
            response.markdown(`Falcon could not answer that request: ${message}`);
        }
    });
    participant.iconPath = new vscode.ThemeIcon("shield");
    participant.followupProvider = {
        provideFollowups() {
            return [
                { prompt: "why did my score drop?", label: "Explain score delta" },
                {
                    prompt: "explain rule avoid-empty-catch",
                    label: "Explain a rule",
                },
            ];
        },
    };
    context.subscriptions.push(participant);
    return participant;
}
function isScoreDropIntent(prompt, command) {
    const lower = prompt.toLowerCase();
    return (command === "score" ||
        lower.includes("score") ||
        lower.includes("drop") ||
        lower.includes("delta"));
}
function parseRuleIntent(prompt, command) {
    const trimmed = prompt.trim();
    if (command === "explain") {
        return firstRuleLikeToken(trimmed);
    }
    const match = trimmed.match(/\bexplain(?:\s+rule)?\s+([a-z0-9][a-z0-9_-]*)\b/i);
    return match?.[1];
}
function firstRuleLikeToken(prompt) {
    return prompt
        .split(/\s+/)
        .map((token) => token.trim())
        .find((token) => /^[a-z0-9][a-z0-9_-]*$/i.test(token));
}
async function explainScoreDrop(context, response, token) {
    const target = resolveScoreTarget();
    if (!target) {
        response.markdown("Open a Dart file or workspace folder before asking for a Falcon score.");
        return;
    }
    response.progress("Running `falcon score --format json`...");
    const score = await runFalconScore(target.fsPath, token);
    const current = scoreHistory(score);
    const key = `${SCORE_HISTORY_KEY}${target.toString()}`;
    const previous = context.workspaceState.get(key);
    await context.workspaceState.update(key, current);
    response.markdown(renderScoreDelta(current, previous));
}
async function explainRule(rule, response, token) {
    response.progress(`Running \`falcon x explain ${rule}\`...`);
    const text = await runFalconCommand(["x", "explain", rule], token);
    const clean = stripAnsi(text).trim();
    if (!clean) {
        response.markdown(`Falcon did not return an explanation for \`${rule}\`.`);
        return;
    }
    response.markdown(["```text", clean, "```"].join("\n"));
}
function resolveScoreTarget() {
    const activeDocument = vscode.window.activeTextEditor?.document;
    if (activeDocument) {
        return (vscode.workspace.getWorkspaceFolder(activeDocument.uri)?.uri ??
            activeDocument.uri);
    }
    return vscode.workspace.workspaceFolders?.[0]?.uri;
}
function runFalconScore(target, token) {
    return runFalconCommand(["score", target, "--format", "json"], token).then((stdout) => JSON.parse(stdout));
}
function runFalconCommand(args, token) {
    const falcon = resolveFalconCliPath();
    const cwd = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath ??
        path.dirname(vscode.window.activeTextEditor?.document.uri.fsPath ?? ".");
    return new Promise((resolve, reject) => {
        const child = cp.execFile(falcon, args, { cwd, timeout: 15000 }, (error, stdout, stderr) => {
            disposable.dispose();
            if (error) {
                reject(new Error(stderr.trim() || error.message));
                return;
            }
            resolve(stdout);
        });
        const disposable = token.onCancellationRequested(() => {
            child.kill();
            reject(new Error("Falcon chat request cancelled."));
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
function scoreHistory(score) {
    return {
        overall: score.overall,
        dimensions: collectDimensionScores(score),
        findings: collectFindings(score),
    };
}
function collectDimensionScores(score) {
    const dimensions = dimensionEntries(score);
    const scores = {};
    for (const [name, dimension] of dimensions) {
        if (typeof dimension?.score === "number") {
            scores[name] = dimension.score;
        }
    }
    return scores;
}
function collectFindings(score) {
    return dimensionEntries(score).flatMap(([name, dimension]) => (dimension?.findings ?? []).map((finding) => `${name}: ${finding}`));
}
function dimensionEntries(score) {
    return [
        ["Resource safety", score.resource_safety],
        ["Error handling", score.error_handling],
        ["Type safety", score.type_safety],
        ["Security", score.security],
        ["Convention match", score.convention_match],
        ["Complexity", score.complexity],
    ];
}
function renderScoreDelta(current, previous) {
    const lines = [`Current Falcon score: **${current.overall}**.`];
    if (!previous) {
        lines.push("No previous chat score is stored for this target yet. Ask again after the next change to see the delta.");
        return appendFindings(lines, current.findings);
    }
    const delta = current.overall - previous.overall;
    const sign = delta > 0 ? "+" : "";
    lines.push(`Delta since the previous chat check: **${sign}${delta}**.`);
    const dimensionDeltas = Object.entries(current.dimensions)
        .map(([name, value]) => [name, value - (previous.dimensions[name] ?? value)])
        .filter(([, value]) => value !== 0)
        .sort((a, b) => a[1] - b[1]);
    if (dimensionDeltas.length > 0) {
        lines.push("", "Changed dimensions:");
        lines.push(...dimensionDeltas.map(([name, value]) => {
            const dimensionSign = value > 0 ? "+" : "";
            return `- ${name}: ${dimensionSign}${value}`;
        }));
    }
    return appendFindings(lines, current.findings);
}
function appendFindings(lines, findings) {
    if (findings.length > 0) {
        lines.push("", "Current contributors:");
        lines.push(...findings.slice(0, 8).map((finding) => `- ${finding}`));
    }
    return lines.join("\n");
}
function stripAnsi(text) {
    return text.replace(/\u001b\[[0-9;]*m/g, "");
}
//# sourceMappingURL=chatParticipant.js.map