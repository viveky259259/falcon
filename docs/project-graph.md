# Project source graph

A directory-level `falcon check PROJECT` (also `analyze`) automatically writes
`PROJECT/.falcon-data/project-graph.json`. Graph generation is independent of lint
severity: an analysis with findings still gets a graph. The artifact is a complete
observation of the configured Dart inventory, including when `--since` limits the
lint report to changed files and dependents.

```sh
falcon check .
falcon check . --graph-output graph.json
falcon check . --no-graph
falcon graph . > graph.json
falcon graph . --output graph.json --config falcon.yaml
```

`falcon graph` writes one JSON document to stdout by default and does not persist
anything. `--output` and `--graph-output` require an existing parent directory.
`--no-graph` skips graph generation and persistence; it conflicts with
`--graph-output`. It does not disable the existing analysis history snapshots.
Single-file checks leave the project artifact untouched unless `--graph-output`
is supplied. A missing root, unreadable source, malformed pubspec, or failed write
returns an error. Graph publication uses a same-directory temporary file and an
atomic rename, so a failed write does not leave a partial replacement.

Add `.falcon-data/project-graph.json` to your project's ignore rules when this
artifact should remain local. Custom export paths follow your project's rules.

## Contents and evidence

The JSON has `schema_version: 1`, `extractor_version`, `snapshot_digest`,
`package_name`, `excluded_patterns`, `nodes`, `edges`, and `diagnostics`.

Nodes represent the project, Dart files, classes, mixins, enums, extensions, type
aliases, functions, methods, constructors, variables and parameters. Symbols
carry their relative path, qualified name, source digest and source span.
Containment connects declarations to their enclosing declaration or file.
Relationships include imports, exports, parts, extends, mixes_in, implements,
and calls.

Containment is observed syntax. Dependency and symbol links are **inferred** from
syntax and the selected inventory; each edge includes its originating file,
source digest and span. Spans use zero-based UTF-8 byte offsets and one-based
lines/byte columns, not LSP UTF-16 character positions.

Imports resolve relative paths and the current package's `package:` URIs from
`pubspec.yaml`. Visibility respects import prefixes, show/hide combinators,
export traversal and part membership. Calls link only uniquely visible bare
function/method names. External SDK/packages, conditional URI selection,
receiver dispatch, callbacks and ambiguous symbols remain diagnostic observations
rather than guessed links. Class hierarchy extraction currently covers class
headers. This graph does not claim complete Dart semantic resolution, runtime
reachability, or resource ownership proof.

Parser recovery emits `syntax_error` diagnostics while retaining extracted facts;
a graph with these diagnostics may have incomplete coverage. External,
unresolved and ambiguous dependencies likewise do not prove that code is absent.
The graph is useful as input to tools and analyses; its inferred edges do not
change existing lint results.

## Identity and scope

IDs are SHA-256 hashes of relative structural identities. For a uniquely named
symbol, inserting lines or changing its body preserves its ID; its source digest
and span change. Duplicate declarations use an occurrence ordinal and remain
ambiguous when resolution cannot select one. Renaming or moving a symbol changes
its ID. Edge IDs include the source occurrence offset, so a line shift can change
an edge ID even when the relationship remains the same.

The snapshot digest hashes extractor version, package name, normalized exclusion
patterns, and sorted file paths/content hashes. Identical selected sources and
configuration produce identical bytes across checkout locations; there are no
absolute paths or timestamps. This is a content observation of disk reads, not a
Git commit pin or an atomic filesystem snapshot. Concurrent edits can produce a
mixed observation; consumers should compare file hashes with their current inputs.

The graph respects `FalconConfig.exclude`. `unused.exclude` applies to unused-code
checks and does not remove declarations from the graph. Unparsed non-Dart assets
are outside this version's inventory. No external indexing service, model, graph
database or network call is required. Generating a complete graph adds a separate
full-inventory parse to a project check, even for incremental linting.

## Embedding

`Falcon::generate_project_graph(&Path)` and
`FalconSdk::generate_project_graph(&str)` return the typed `ProjectGraph` without
writing files. Existing analysis result structures and methods are unchanged.
`ProjectGraph::write_to(&Path)` exports it atomically;
`ProjectGraph::save_project(&Path)` creates the conventional artifact directory.
Automatic persistence is a CLI `check` behavior; SDK, MCP, HTTP, LSP and other
commands retain their existing behavior.
