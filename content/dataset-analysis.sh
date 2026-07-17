#!/bin/bash
# =============================================================================
# Falcon Dataset Analysis Script
# Analyze AI-generated Flutter files across multiple projects/tools
#
# Usage:
#   ./dataset-analysis.sh /path/to/projects/dir [output-dir]
#
# The script:
#   1. Finds all Flutter projects in the given directory
#   2. Runs falcon ai-score, provenance, and discover-rules on each
#   3. Records each into the benchmark database
#   4. Generates an aggregated markdown report
# =============================================================================

set -euo pipefail

PROJECTS_DIR="${1:-.}"
OUTPUT_DIR="${2:-./falcon-dataset-report}"
TOOL="${FALCON_AI_TOOL:-unknown}"

mkdir -p "$OUTPUT_DIR"

echo "================================================"
echo "  Falcon Dataset Analysis"
echo "  Projects dir: $PROJECTS_DIR"
echo "  AI Tool:      $TOOL"
echo "  Output:       $OUTPUT_DIR"
echo "================================================"
echo ""

REPORT_FILE="$OUTPUT_DIR/dataset-report.md"
cat > "$REPORT_FILE" << 'HEADER'
# Falcon Dataset Analysis Report

| Project | Score | Grade | Files | Issues | Errors | Warnings |
|---|---|---|---|---|---|---|
HEADER

PROJECT_COUNT=0
TOTAL_SCORE=0
TOTAL_FILES=0
TOTAL_ISSUES=0

for project in "$PROJECTS_DIR"/*/; do
    if [ ! -f "$project/pubspec.yaml" ]; then
        continue
    fi

    PROJECT_NAME=$(basename "$project")
    echo "▸ Analyzing: $PROJECT_NAME"

    # Run ai-score
    SCORE_JSON=$(falcon ai-score "$project" --json 2>/dev/null || echo '{"overall":0,"grade":"?","file_count":0,"total_issues":0}')

    SCORE=$(echo "$SCORE_JSON" | grep -o '"overall":[0-9]*' | head -1 | cut -d: -f2)
    GRADE=$(echo "$SCORE_JSON" | grep -o '"grade":"[^"]*"' | head -1 | cut -d'"' -f4)
    FILES=$(echo "$SCORE_JSON" | grep -o '"file_count":[0-9]*' | head -1 | cut -d: -f2)
    ISSUES=$(echo "$SCORE_JSON" | grep -o '"total_issues":[0-9]*' | head -1 | cut -d: -f2)

    SCORE=${SCORE:-0}
    GRADE=${GRADE:-?}
    FILES=${FILES:-0}
    ISSUES=${ISSUES:-0}

    # Count errors and warnings
    ERRORS=$(falcon analyze "$project" --format json 2>/dev/null | grep -c '"Error"' || echo "0")
    WARNINGS=$(falcon analyze "$project" --format json 2>/dev/null | grep -c '"Warning"' || echo "0")

    # Record to benchmark database
    falcon x benchmark-db "$project" --tool "$TOOL" 2>/dev/null || true

    # Record to cross-project learning
    falcon x learn "$project" 2>/dev/null || true

    # Add to report
    echo "| $PROJECT_NAME | $SCORE | $GRADE | $FILES | $ISSUES | $ERRORS | $WARNINGS |" >> "$REPORT_FILE"

    PROJECT_COUNT=$((PROJECT_COUNT + 1))
    TOTAL_SCORE=$((TOTAL_SCORE + SCORE))
    TOTAL_FILES=$((TOTAL_FILES + FILES))
    TOTAL_ISSUES=$((TOTAL_ISSUES + ISSUES))

    echo "  Score: $SCORE/100 ($GRADE) | $FILES files | $ISSUES issues"
done

# Calculate average
if [ $PROJECT_COUNT -gt 0 ]; then
    AVG_SCORE=$((TOTAL_SCORE / PROJECT_COUNT))
else
    AVG_SCORE=0
fi

# Append summary
cat >> "$REPORT_FILE" << EOF

## Summary

- **Projects analyzed**: $PROJECT_COUNT
- **Average AI Score**: $AVG_SCORE/100
- **Total files**: $TOTAL_FILES
- **Total issues**: $TOTAL_ISSUES
- **AI Tool**: $TOOL

## Cross-Project Insights

EOF

# Add cross-project insights
falcon x learn --insights 2>/dev/null >> "$REPORT_FILE" || true

# Add benchmark summary
echo "## Benchmark Database" >> "$REPORT_FILE"
echo "" >> "$REPORT_FILE"
falcon x benchmark-db --summary 2>/dev/null >> "$REPORT_FILE" || true

echo ""
echo "================================================"
echo "  Analysis complete!"
echo "  Projects: $PROJECT_COUNT"
echo "  Average score: $AVG_SCORE/100"
echo "  Report: $REPORT_FILE"
echo "================================================"
