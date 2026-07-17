import * as cp from "child_process";
import * as path from "path";
import * as vscode from "vscode";

type DimensionScore = {
  score?: number;
  findings?: string[];
};

type FalconScore = {
  overall: number;
  resource_safety?: DimensionScore;
  error_handling?: DimensionScore;
  type_safety?: DimensionScore;
  security?: DimensionScore;
  convention_match?: DimensionScore;
  complexity?: DimensionScore;
};

type ScoreSnapshot = {
  overall: number;
  findings: string[];
  recordedAt: number;
};

type Runner = (document: vscode.TextDocument) => Promise<FalconScore>;

const HISTORY_PREFIX = "falcon.scoreLens.history.";
const DEBOUNCE_MS = 250;

class ScoreCodeLens extends vscode.CodeLens {
  constructor(range: vscode.Range, readonly uri: vscode.Uri) {
    super(range);
  }
}

export class ScoreLensProvider implements vscode.CodeLensProvider {
  private readonly onDidChangeEmitter = new vscode.EventEmitter<void>();
  readonly onDidChangeCodeLenses = this.onDidChangeEmitter.event;
  private readonly pending = new Map<string, Promise<vscode.CodeLens>>();
  private readonly memory = new Map<string, ScoreSnapshot>();

  constructor(
    private readonly context: vscode.ExtensionContext,
    private readonly outputChannel: vscode.OutputChannel,
    private readonly runner: Runner = runFalconScore
  ) {}

  refresh() {
    this.onDidChangeEmitter.fire();
  }

  provideCodeLenses(document: vscode.TextDocument): vscode.CodeLens[] {
    if (!shouldShowScoreLens(document)) {
      return [];
    }

    return [new ScoreCodeLens(new vscode.Range(0, 0, 0, 0), document.uri)];
  }

  async resolveCodeLens(codeLens: vscode.CodeLens): Promise<vscode.CodeLens> {
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

  private async resolveForDocument(
    codeLens: vscode.CodeLens,
    document: vscode.TextDocument
  ): Promise<vscode.CodeLens> {
    await delay(DEBOUNCE_MS);

    try {
      const previous = this.snapshotFor(document);
      const score = await this.runner(document);
      const current = scoreSnapshot(score);
      await this.remember(document, current);

      const delta =
        previous === undefined ? undefined : current.overall - previous.overall;
      const title = scoreLensTitle(current.overall, delta);
      codeLens.command = {
        title,
        command: "falcon.showOutput",
        tooltip: scoreLensTooltip(current, previous),
      };
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      this.outputChannel.appendLine(`Falcon score lens failed: ${message}`);
      codeLens.command = disabledCommand("Falcon score unavailable");
    }

    return codeLens;
  }

  private snapshotFor(document: vscode.TextDocument): ScoreSnapshot | undefined {
    const key = historyKey(document);
    return (
      this.memory.get(key) ??
      this.context.workspaceState.get<ScoreSnapshot>(key)
    );
  }

  private async remember(document: vscode.TextDocument, snapshot: ScoreSnapshot) {
    const key = historyKey(document);
    this.memory.set(key, snapshot);
    await this.context.workspaceState.update(key, snapshot);
  }
}

export function shouldShowScoreLens(document: vscode.TextDocument): boolean {
  const config = vscode.workspace.getConfiguration("falcon");
  return (
    document.languageId === "dart" &&
    config.get<boolean>("enable", true) &&
    config.get<boolean>("scoreLens.enabled", true) &&
    !config.get<boolean>("quietMode", false)
  );
}

export function scoreSnapshot(score: FalconScore): ScoreSnapshot {
  return {
    overall: score.overall,
    findings: collectFindings(score),
    recordedAt: Date.now(),
  };
}

export function scoreLensTitle(score: number, delta: number | undefined): string {
  if (delta === undefined) {
    return `Falcon score ${score}`;
  }
  if (delta === 0) {
    return `Falcon score ${score} (no change)`;
  }
  const sign = delta > 0 ? "+" : "";
  return `Falcon score ${score} (${sign}${delta})`;
}

function scoreLensTooltip(
  current: ScoreSnapshot,
  previous: ScoreSnapshot | undefined
): string {
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

function disabledCommand(title: string): vscode.Command {
  return {
    title,
    command: "falcon.showOutput",
  };
}

function collectFindings(score: FalconScore): string[] {
  const dimensions = [
    ["Resource safety", score.resource_safety],
    ["Error handling", score.error_handling],
    ["Type safety", score.type_safety],
    ["Security", score.security],
    ["Convention match", score.convention_match],
    ["Complexity", score.complexity],
  ] as const;

  return dimensions.flatMap(([name, dimension]) =>
    (dimension?.findings ?? []).map((finding) => `${name}: ${finding}`)
  );
}

function runFalconScore(document: vscode.TextDocument): Promise<FalconScore> {
  const workspaceFolder = vscode.workspace.getWorkspaceFolder(document.uri);
  const cwd = workspaceFolder?.uri.fsPath ?? path.dirname(document.uri.fsPath);
  const target = workspaceFolder?.uri.fsPath ?? document.uri.fsPath;
  const falcon = resolveFalconCliPath();

  return new Promise((resolve, reject) => {
    cp.execFile(
      falcon,
      ["score", target, "--format", "json"],
      { cwd, timeout: 15000 },
      (error, stdout, stderr) => {
        if (error) {
          reject(new Error(stderr.trim() || error.message));
          return;
        }

        try {
          resolve(JSON.parse(stdout) as FalconScore);
        } catch (parseError) {
          reject(parseError);
        }
      }
    );
  });
}

function resolveFalconCliPath(): string {
  const configured = vscode.workspace
    .getConfiguration("falcon")
    .get<string>("executablePath", "");

  if (!configured) {
    return "falcon";
  }

  const base = path.basename(configured);
  if (base === "falcon-lsp" || base === "falcon-lsp.exe") {
    return path.join(
      path.dirname(configured),
      process.platform === "win32" ? "falcon.exe" : "falcon"
    );
  }

  return configured;
}

function historyKey(document: vscode.TextDocument): string {
  return `${HISTORY_PREFIX}${document.uri.toString()}`;
}

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
