import * as vscode from "vscode";
import { HandleDiagnosticsSignature } from "vscode-languageclient";

/**
 * The diagnostic `source` value Falcon stamps on every diagnostic it emits.
 * Used both to identify Falcon's own diagnostics and to recognise (and
 * therefore ignore) diagnostics owned by other publishers such as Dart-Code.
 */
export const FALCON_SOURCE = "falcon";

/**
 * Prefix prepended to every Falcon rule id when surfaced as a VS Code
 * diagnostic `code`. The Problems panel will display `falcon/<rule-id>` so it
 * is unambiguous which tool emitted a given finding.
 */
export const FALCON_CODE_PREFIX = "falcon/";

/**
 * Build the `handleDiagnostics` middleware that:
 *   1. Stamps every incoming Falcon diagnostic with `source = "falcon"` and a
 *      namespaced `code` of the form `falcon/<rule-id>` (preserving any
 *      existing `target` URI so the Problems panel link still works).
 *   2. Optionally drops info-level diagnostics when `falcon.quietMode` is on.
 *   3. Suppresses Falcon diagnostics on a `(uri, line)` pair when a non-Falcon
 *      diagnostic (e.g. one published by Dart-Code's analyzer) is already
 *      present on that same line, unless `falcon.deferToAnalyzer` is `false`.
 *
 * NOTE: The middleware runs *before* VS Code's own diagnostic store has been
 * updated for the current publish, so on the very first publish for a file
 * Dart-Code's diagnostics may not yet be visible to
 * `vscode.languages.getDiagnostics`. That's acceptable — every subsequent
 * publish self-corrects as the Dart analyzer settles and re-publishes.
 */
export function buildHandleDiagnosticsMiddleware(): (
  this: void,
  uri: vscode.Uri,
  diagnostics: vscode.Diagnostic[],
  next: HandleDiagnosticsSignature
) => void {
  return (uri, diagnostics, next) => {
    const config = vscode.workspace.getConfiguration("falcon");
    const deferToAnalyzer = config.get<boolean>("deferToAnalyzer", true);
    const quietMode = config.get<boolean>("quietMode", false);

    // 1. Stamp namespaced source/code on every Falcon diagnostic.
    for (const diag of diagnostics) {
      stampFalconDiagnostic(diag);
    }

    // 2. Optional quiet-mode filter: drop Falcon info/hint diagnostics.
    let filtered = quietMode
      ? diagnostics.filter(
          (d) =>
            d.severity !== vscode.DiagnosticSeverity.Information &&
            d.severity !== vscode.DiagnosticSeverity.Hint
        )
      : diagnostics;

    // 3. Collision suppression: drop Falcon diagnostics on lines already
    //    occupied by a non-Falcon diagnostic (e.g. Dart-Code analyzer).
    if (deferToAnalyzer) {
      const occupiedLines = collectNonFalconLines(uri);
      if (occupiedLines.size > 0) {
        filtered = filtered.filter((d) => {
          if (d.source !== FALCON_SOURCE) return true;
          return !occupiedLines.has(d.range.start.line);
        });
      }
    }

    next(uri, filtered);
  };
}

/**
 * Mutates `diag` so that:
 *   - `source` is `"falcon"`.
 *   - `code` reads as `falcon/<rule-id>` in the Problems panel. If `code` was
 *     a `{ value, target }` object, the `target` URI is preserved so the link
 *     in the Problems panel remains clickable.
 *
 * Idempotent: a diagnostic already correctly stamped is left untouched.
 */
function stampFalconDiagnostic(diag: vscode.Diagnostic): void {
  diag.source = FALCON_SOURCE;

  const existing = diag.code;
  if (existing === undefined || existing === null) {
    return;
  }

  if (typeof existing === "string" || typeof existing === "number") {
    const asString = String(existing);
    if (asString.startsWith(FALCON_CODE_PREFIX)) return;
    diag.code = FALCON_CODE_PREFIX + asString;
    return;
  }

  // `code` is a `{ value, target }` object — preserve the target URI.
  const value = String(existing.value);
  if (value.startsWith(FALCON_CODE_PREFIX)) return;
  diag.code = {
    value: FALCON_CODE_PREFIX + value,
    target: existing.target,
  };
}

/**
 * Collect the set of line numbers (zero-based) in `uri` that already host at
 * least one non-Falcon diagnostic. Used to decide which incoming Falcon
 * diagnostics to suppress under the coexistence contract.
 */
function collectNonFalconLines(uri: vscode.Uri): Set<number> {
  const lines = new Set<number>();
  const existing = vscode.languages.getDiagnostics(uri);
  for (const d of existing) {
    if (d.source === FALCON_SOURCE) continue;
    for (let line = d.range.start.line; line <= d.range.end.line; line++) {
      lines.add(line);
    }
  }
  return lines;
}
