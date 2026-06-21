#!/bin/bash
# Cleanup script for public release
set -e

echo "🧹 Starting repository cleanup for public release..."
echo

# Change to repo root
cd "$(dirname "$0")/.."

# Count files before cleanup
echo "📊 Files before cleanup:"
echo "  Root .md files: $(ls -1 *.md 2>/dev/null | wc -l)"
echo "  Root .py files: $(ls -1 *.py 2>/dev/null | wc -l)"
echo "  Root .log files: $(ls -1 *.log 2>/dev/null | wc -l)"
echo "  Root .json files: $(ls -1 *.json 2>/dev/null | wc -l)"
echo

# Files to KEEP in root
KEEP_FILES=(
    "README.md"
    "CHANGELOG.md"
    "CONTRIBUTING.md"
    "AGENTS.md"
    "Cargo.toml"
    "Cargo.lock"
    "pyproject.toml"
    "Makefile"
    ".gitignore"
    ".editorconfig"
    "rustfmt.toml"
    "clippy.toml"
    "deny.toml"
    "LICENSE-MIT"
    "LICENSE-APACHE"
)

# Step 1: Remove development markdown files
echo "🗑️  Step 1: Removing development markdown files..."
for file in *.md; do
    # Check if file should be kept
    keep=false
    for keep_file in "${KEEP_FILES[@]}"; do
        if [ "$file" = "$keep_file" ]; then
            keep=true
            break
        fi
    done

    if [ "$keep" = false ] && [ -f "$file" ]; then
        echo "  Removing: $file"
        git rm "$file" 2>/dev/null || rm "$file"
    fi
done

# Step 2: Remove Python comparison scripts
echo
echo "🗑️  Step 2: Removing Python comparison scripts..."
for file in *.py; do
    if [ -f "$file" ]; then
        echo "  Removing: $file"
        git rm "$file" 2>/dev/null || rm "$file"
    fi
done

# Step 3: Remove log files
echo
echo "🗑️  Step 3: Removing log files..."
for file in *.log; do
    if [ -f "$file" ]; then
        echo "  Removing: $file"
        rm "$file"  # Don't use git rm - these are gitignored
    fi
done

# Step 4: Remove JSON analysis files
echo
echo "🗑️  Step 4: Removing JSON analysis files..."
for file in *.json; do
    if [ -f "$file" ]; then
        echo "  Removing: $file"
        rm "$file"  # Don't use git rm - may be gitignored
    fi
done

# Step 5: Remove temporary files
echo
echo "🗑️  Step 5: Removing temporary files..."
[ -f "cleanup_and_rerun.sh" ] && git rm "cleanup_and_rerun.sh" 2>/dev/null || rm -f "cleanup_and_rerun.sh"
[ -f "test_spacing_fix.sh" ] && git rm "test_spacing_fix.sh" 2>/dev/null || rm -f "test_spacing_fix.sh"
[ -f "debug_word_splitting.md" ] && git rm "debug_word_splitting.md" 2>/dev/null || rm -f "debug_word_splitting.md"
[ -f "span_test.txt" ] && rm -f "span_test.txt"
[ -f "LICENSE.old" ] && git rm "LICENSE.old" 2>/dev/null || rm -f "LICENSE.old"
[ -f "debug_xy_cut" ] && rm -f "debug_xy_cut"
[ -f "quality_analysis_output.txt" ] && rm -f "quality_analysis_output.txt"
[ -f "quality_analysis_report.txt" ] && rm -f "quality_analysis_report.txt"

# Step 6: Clean docs/development/sessions/
echo
echo "🗑️  Step 6: Cleaning docs/development/sessions/..."
if [ -d "docs/development/sessions" ]; then
    git rm -r "docs/development/sessions/" 2>/dev/null || rm -rf "docs/development/sessions/"
fi

# Step 7: Clean docs/issues/
echo
echo "🗑️  Step 7: Cleaning docs/issues/..."
if [ -d "docs/issues" ]; then
    git rm -r "docs/issues/" 2>/dev/null || rm -rf "docs/issues/"
fi

# Step 8: Prune docs/quality/
echo
echo "🗑️  Step 8: Pruning docs/quality/..."
# Remove all but final summary
if [ -d "docs/quality/improvements" ]; then
    find docs/quality/improvements/ -type f -exec git rm {} \; 2>/dev/null || find docs/quality/improvements/ -type f -delete
fi
if [ -d "docs/quality/comparisons" ]; then
    find docs/quality/comparisons/ -type f -exec git rm {} \; 2>/dev/null || find docs/quality/comparisons/ -type f -delete
fi
# Keep only FINAL_QUALITY_IMPROVEMENTS_SUMMARY.md in summaries
if [ -d "docs/quality/summaries" ]; then
    for file in docs/quality/summaries/*; do
        if [ -f "$file" ] && [ "$(basename "$file")" != "FINAL_QUALITY_IMPROVEMENTS_SUMMARY.md" ]; then
            git rm "$file" 2>/dev/null || rm "$file"
        fi
    done
fi

# Step 9: Remove docs/PRE_PUBLICATION_CLEANUP.md
echo
echo "🗑️  Step 9: Removing docs/PRE_PUBLICATION_CLEANUP.md..."
[ -f "docs/PRE_PUBLICATION_CLEANUP.md" ] && git rm "docs/PRE_PUBLICATION_CLEANUP.md" 2>/dev/null || rm -f "docs/PRE_PUBLICATION_CLEANUP.md"

echo
echo "✅ Cleanup complete!"
echo
echo "📊 Summary:"
echo "  Root .md files remaining: $(ls -1 *.md 2>/dev/null | wc -l)"
echo "  Root .py files remaining: $(ls -1 *.py 2>/dev/null | wc -l)"
echo "  Root .log files remaining: $(ls -1 *.log 2>/dev/null | wc -l)"
echo "  Root .json files remaining: $(ls -1 *.json 2>/dev/null | wc -l)"
echo
echo "📝 Next steps:"
echo "  1. Review changes: git status"
echo "  2. Create essential files (CODE_OF_CONDUCT.md, etc.)"
echo "  3. Reorganize structure (scripts/, examples/)"
echo "  4. Update existing files (README.md, CONTRIBUTING.md)"
echo "  5. Commit changes: git commit -m 'chore: Clean repository for public release'"
