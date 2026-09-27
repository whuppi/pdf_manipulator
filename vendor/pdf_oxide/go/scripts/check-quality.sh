#!/bin/bash
# PDF Oxide Go - Code Quality & Security Check Script
# Run all linting, formatting, and security checks locally
# Usage: ./scripts/check-quality.sh [--fix]

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
FIX_MODE="${1:-}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Counter for issues
ISSUES_FOUND=0

# Helper functions
print_header() {
    echo -e "\n${BLUE}═══════════════════════════════════════════════════════════════${NC}"
    echo -e "${BLUE}$1${NC}"
    echo -e "${BLUE}═══════════════════════════════════════════════════════════════${NC}\n"
}

print_check() {
    echo -e "${YELLOW}▶ $1${NC}"
}

print_success() {
    echo -e "${GREEN}✓ $1${NC}"
}

print_error() {
    echo -e "${RED}✗ $1${NC}"
    ((ISSUES_FOUND++))
}

print_info() {
    echo -e "${BLUE}ℹ $1${NC}"
}

# Check prerequisites
check_prerequisites() {
    print_header "Checking Prerequisites"

    print_check "Checking Go installation..."
    if ! command -v go &> /dev/null; then
        print_error "Go is not installed"
        exit 1
    fi
    GO_VERSION=$(go version | awk '{print $3}')
    print_success "Go $GO_VERSION found"

    print_check "Checking Go module..."
    if [ ! -f "$PROJECT_DIR/go.mod" ]; then
        print_error "go.mod not found"
        exit 1
    fi
    print_success "go.mod found"

    # Check for optional tools
    print_info "Optional tools:"

    if command -v golangci-lint &> /dev/null; then
        GOLANGCI_VERSION=$(golangci-lint --version 2>/dev/null | head -1)
        print_success "golangci-lint: $GOLANGCI_VERSION"
    else
        print_info "Install golangci-lint: https://golangci-lint.run/usage/install/"
    fi

    if command -v gosec &> /dev/null; then
        print_success "gosec found"
    else
        print_info "Install gosec: go install github.com/securego/gosec/v2/cmd/gosec@latest"
    fi

    if command -v goimports &> /dev/null; then
        print_success "goimports found"
    else
        print_info "Install goimports: go install golang.org/x/tools/cmd/goimports@latest"
    fi
}

# Format check
check_format() {
    print_header "Checking Code Format"

    print_check "Running gofmt..."
    if gofmt -l . | grep -q "\.go"; then
        if [ "$FIX_MODE" == "--fix" ]; then
            print_info "Fixing format issues..."
            gofmt -w .
            print_success "Format issues fixed"
        else
            print_error "Format issues found (run with --fix to auto-fix)"
            gofmt -l .
        fi
    else
        print_success "Code formatting is correct"
    fi

    print_check "Checking imports..."
    if command -v goimports &> /dev/null; then
        if goimports -l . | grep -q "\.go"; then
            if [ "$FIX_MODE" == "--fix" ]; then
                print_info "Fixing import issues..."
                goimports -w .
                print_success "Import issues fixed"
            else
                print_error "Import issues found (run with --fix to auto-fix)"
                goimports -l .
            fi
        else
            print_success "Imports are properly organized"
        fi
    else
        print_info "goimports not installed, skipping import check"
    fi
}

# Lint check
check_lint() {
    print_header "Checking Code Quality (Linting)"

    print_check "Running golangci-lint..."
    if command -v golangci-lint &> /dev/null; then
        if golangci-lint run --timeout=5m; then
            print_success "No linting issues found"
        else
            print_error "Linting issues found"
        fi
    else
        print_info "golangci-lint not installed"
        echo "Install: https://golangci-lint.run/usage/install/"

        # Fall back to basic checks
        print_info "Running basic checks with go vet..."
        if go vet ./...; then
            print_success "No issues detected by go vet"
        else
            print_error "Issues detected by go vet"
        fi
    fi
}

# Vet check
check_vet() {
    print_header "Running Go Vet"

    print_check "Analyzing code with go vet..."
    if go vet ./...; then
        print_success "No suspicious constructs found"
    else
        print_error "Suspicious constructs detected"
    fi
}

# Type check
check_types() {
    print_header "Type Checking"

    print_check "Building to check types..."
    if go build ./...; then
        print_success "Type checking passed"
    else
        print_error "Type checking failed"
    fi
}

# Error check
check_errors() {
    print_header "Error Handling Check"

    print_check "Checking for unchecked errors..."
    if command -v errcheck &> /dev/null; then
        if errcheck -ignorepkg=fmt ./...; then
            print_success "All errors are properly handled"
        else
            print_error "Unchecked errors found"
        fi
    else
        print_info "errcheck not installed"
        echo "Install: go install github.com/kisielk/errcheck@latest"
    fi
}

# Security check
check_security() {
    print_header "Security Analysis"

    print_check "Running gosec..."
    if command -v gosec &> /dev/null; then
        if gosec -quiet ./... 2>/dev/null; then
            print_success "No security issues found"
        else
            print_error "Security issues detected"
        fi
    else
        print_info "gosec not installed"
        echo "Install: go install github.com/securego/gosec/v2/cmd/gosec@latest"
    fi

    print_check "Checking for vulnerable dependencies..."
    if command -v govulncheck &> /dev/null; then
        if govulncheck ./... 2>/dev/null; then
            print_success "No vulnerable dependencies found"
        else
            print_error "Vulnerable dependencies detected"
        fi
    else
        print_info "govulncheck not installed"
        echo "Install: go install golang.org/x/vuln/cmd/govulncheck@latest"
    fi
}

# Test coverage
check_coverage() {
    print_header "Test Coverage"

    print_check "Running tests with coverage..."
    if go test ./... -cover -coverprofile=coverage.out; then
        print_success "Tests passed"

        COVERAGE=$(go tool cover -func=coverage.out | grep total | awk '{print $3}')
        print_info "Total coverage: $COVERAGE"

        # Generate HTML report
        if command -v go &> /dev/null; then
            go tool cover -html=coverage.out -o coverage.html
            print_success "Coverage report generated: coverage.html"
        fi
    else
        print_error "Tests failed"
    fi

    # Cleanup
    rm -f coverage.out
}

# Complexity analysis
check_complexity() {
    print_header "Code Complexity Analysis"

    print_check "Checking cyclomatic complexity..."
    if command -v gocyclo &> /dev/null; then
        if gocyclo -over 15 ./... > /dev/null 2>&1; then
            print_info "High complexity functions found (>15)"
            gocyclo -over 15 ./...
        else
            print_success "No functions with excessive complexity (>15)"
        fi
    else
        print_info "gocyclo not installed"
        echo "Install: go install github.com/fzipp/gocyclo/cmd/gocyclo@latest"
    fi
}

# Dependencies check
check_dependencies() {
    print_header "Dependency Management"

    print_check "Checking go.mod and go.sum..."
    if go mod verify; then
        print_success "Module files are consistent"
    else
        print_error "Module file inconsistencies found"
    fi

    print_check "Tidying go.mod..."
    if go mod tidy; then
        print_success "go.mod is tidy"
    else
        print_error "Failed to tidy go.mod"
    fi

    print_check "Checking for unused dependencies..."
    if command -v go-mod-unused &> /dev/null; then
        if go-mod-unused ./...; then
            print_success "No unused dependencies"
        else
            print_error "Unused dependencies found"
        fi
    else
        print_info "go-mod-unused not installed"
    fi
}

# Documentation check
check_documentation() {
    print_header "Documentation Check"

    print_check "Checking godoc comments..."
    MISSING_DOCS=$(go doc -all 2>&1 | grep -c "undocumented" || true)

    if [ "$MISSING_DOCS" -eq 0 ]; then
        print_success "All exported items are documented"
    else
        print_info "$MISSING_DOCS items may be missing godoc comments"
    fi
}

# Summary
print_summary() {
    print_header "Summary"

    if [ $ISSUES_FOUND -eq 0 ]; then
        echo -e "${GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
        echo -e "${GREEN}✓ All checks passed! Code is production-ready.${NC}"
        echo -e "${GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
        return 0
    else
        echo -e "${RED}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
        echo -e "${RED}✗ Found $ISSUES_FOUND issue(s). Please fix before committing.${NC}"
        echo -e "${RED}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
        return 1
    fi
}

# Main execution
main() {
    cd "$PROJECT_DIR"

    echo -e "${BLUE}"
    echo "╔═══════════════════════════════════════════════════════════════╗"
    echo "║          PDF Oxide Go - Code Quality & Security Check         ║"
    echo "╚═══════════════════════════════════════════════════════════════╝"
    echo -e "${NC}"

    if [ "$FIX_MODE" == "--fix" ]; then
        print_info "Running in FIX mode - will auto-fix issues where possible"
    fi

    check_prerequisites
    check_format
    check_lint
    check_vet
    check_types
    check_errors
    check_security
    check_coverage
    check_complexity
    check_dependencies
    check_documentation

    print_summary
}

# Run main
main
