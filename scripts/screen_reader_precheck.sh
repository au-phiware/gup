#!/usr/bin/env bash
# Screen Reader Testing Pre-Check Script
# 
# This script validates that the test environment is ready for screen reader testing.
# It checks:
# - Example is built
# - Server is running
# - Automated accessibility checks pass
# - ARIA attributes are present
#
# Usage: ./scripts/screen_reader_precheck.sh [example_name]

set -euo pipefail

EXAMPLE="${1:-web_accessibility_demo}"
PORT="${2:-8080}"
URL="http://localhost:${PORT}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Track overall status
WARNINGS=0
ERRORS=0

echo -e "${BLUE}╔═══════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}║  Gup Screen Reader Testing Pre-Check         ║${NC}"
echo -e "${BLUE}╚═══════════════════════════════════════════════╝${NC}"
echo ""
echo "Example: $EXAMPLE"
echo "URL: $URL"
echo ""

# Check 1: WASM package exists
echo -e "${BLUE}[1/7]${NC} Checking if example is built..."
if [ -d "pkg/${EXAMPLE}" ]; then
    echo -e "  ${GREEN}✓${NC} WASM package found"
else
    echo -e "  ${RED}✗${NC} WASM package not found"
    echo -e "  ${YELLOW}→${NC} Run: wasm-pack build --target web --out-dir ../pkg/${EXAMPLE} examples/${EXAMPLE}"
    ((ERRORS++))
fi
echo ""

# Check 2: Server is running
echo -e "${BLUE}[2/7]${NC} Checking if server is running..."
if curl -s -o /dev/null -w "%{http_code}" "$URL" | grep -q "200\|404"; then
    echo -e "  ${GREEN}✓${NC} Server is reachable at $URL"
else
    echo -e "  ${YELLOW}⚠${NC} Server not reachable at $URL"
    echo -e "  ${YELLOW}→${NC} Start server: mask serve ${EXAMPLE}"
    ((WARNINGS++))
fi
echo ""

# Check 3: Accessibility features enabled
echo -e "${BLUE}[3/7]${NC} Checking source code for accessibility features..."
if grep -q "AccessibilitySystem" "examples/${EXAMPLE}.rs" 2>/dev/null; then
    echo -e "  ${GREEN}✓${NC} AccessibilitySystem is imported"
else
    echo -e "  ${RED}✗${NC} AccessibilitySystem not found in example"
    echo -e "  ${YELLOW}→${NC} Example may not have accessibility support"
    ((ERRORS++))
fi

if grep -q "aria_tree" "examples/${EXAMPLE}.rs" 2>/dev/null; then
    echo -e "  ${GREEN}✓${NC} ARIA tree usage detected"
else
    echo -e "  ${YELLOW}⚠${NC} ARIA tree usage not detected"
    ((WARNINGS++))
fi
echo ""

# Check 4: Basic HTML structure (if server is running)
echo -e "${BLUE}[4/7]${NC} Checking HTML structure..."
if command -v curl &> /dev/null; then
    HTML=$(curl -s "$URL" 2>/dev/null || echo "")
    if [ -n "$HTML" ]; then
        # Check for canvas
        if echo "$HTML" | grep -q "<canvas"; then
            echo -e "  ${GREEN}✓${NC} Canvas element found"
        else
            echo -e "  ${YELLOW}⚠${NC} Canvas element not found"
            ((WARNINGS++))
        fi
        
        # Check for overlay container
        if echo "$HTML" | grep -q "gup-overlay-container"; then
            echo -e "  ${GREEN}✓${NC} Overlay container found"
        else
            echo -e "  ${YELLOW}⚠${NC} Overlay container not found (may be added dynamically)"
        fi
        
        # Check for ARIA attributes
        if echo "$HTML" | grep -q "aria-"; then
            echo -e "  ${GREEN}✓${NC} ARIA attributes present"
        else
            echo -e "  ${YELLOW}⚠${NC} ARIA attributes not found in initial HTML (may be added dynamically)"
        fi
    else
        echo -e "  ${YELLOW}⚠${NC} Could not fetch HTML (server may not be running)"
        ((WARNINGS++))
    fi
else
    echo -e "  ${YELLOW}⚠${NC} curl not available, skipping HTML checks"
    ((WARNINGS++))
fi
echo ""

# Check 5: Rust compilation
echo -e "${BLUE}[5/7]${NC} Checking Rust compilation..."
if cargo check --example "$EXAMPLE" --target wasm32-unknown-unknown --quiet 2>&1 | grep -q "error"; then
    echo -e "  ${RED}✗${NC} Example has compilation errors"
    echo -e "  ${YELLOW}→${NC} Run: cargo check --example ${EXAMPLE} --target wasm32-unknown-unknown"
    ((ERRORS++))
else
    echo -e "  ${GREEN}✓${NC} Example compiles successfully"
fi
echo ""

# Check 6: Documentation exists
echo -e "${BLUE}[6/7]${NC} Checking testing documentation..."
DOCS=(
    "docs/SCREEN_READER_TESTING.md"
    "docs/ACCESSIBILITY_COMPATIBILITY.md"
    "docs/ACCESSIBILITY_KNOWN_ISSUES.md"
    "docs/SCREEN_READER_CHECKLIST.md"
)

for doc in "${DOCS[@]}"; do
    if [ -f "$doc" ]; then
        echo -e "  ${GREEN}✓${NC} $doc"
    else
        echo -e "  ${YELLOW}⚠${NC} $doc not found"
        ((WARNINGS++))
    fi
done
echo ""

# Check 7: Automated accessibility check (if server running and tools available)
echo -e "${BLUE}[7/7]${NC} Running automated accessibility checks..."

# Try axe-core if available (npm package)
if command -v npx &> /dev/null && curl -s "$URL" &> /dev/null; then
    echo -e "  ${BLUE}→${NC} Attempting to run axe-core (this may take a moment)..."
    if npx @axe-core/cli "$URL" --exit 2>&1 | tee /tmp/axe-output.txt | grep -q "0 violations"; then
        echo -e "  ${GREEN}✓${NC} axe-core: No violations found"
    else
        if grep -q "violations" /tmp/axe-output.txt; then
            VIOLATION_COUNT=$(grep -o "[0-9]\+ violations" /tmp/axe-output.txt | grep -o "[0-9]\+" || echo "unknown")
            echo -e "  ${YELLOW}⚠${NC} axe-core: $VIOLATION_COUNT violations found"
            echo -e "  ${YELLOW}→${NC} Review violations before manual testing"
            ((WARNINGS++))
        else
            echo -e "  ${YELLOW}⚠${NC} Could not run axe-core (may need npm install)"
        fi
    fi
    rm -f /tmp/axe-output.txt
else
    echo -e "  ${YELLOW}⚠${NC} Automated checks skipped (npx not available or server not running)"
    echo -e "  ${YELLOW}→${NC} Install: npm install -g @axe-core/cli"
fi
echo ""

# Summary
echo -e "${BLUE}═══════════════════════════════════════════════${NC}"
echo -e "${BLUE}Summary${NC}"
echo -e "${BLUE}═══════════════════════════════════════════════${NC}"

if [ $ERRORS -eq 0 ] && [ $WARNINGS -eq 0 ]; then
    echo -e "${GREEN}✓ All checks passed!${NC}"
    echo -e "${GREEN}  Environment is ready for screen reader testing.${NC}"
    echo ""
    echo -e "Next steps:"
    echo -e "  1. Review testing guide: ${BLUE}docs/SCREEN_READER_TESTING.md${NC}"
    echo -e "  2. Print checklist: ${BLUE}docs/SCREEN_READER_CHECKLIST.md${NC}"
    echo -e "  3. Open $URL in browser with screen reader enabled"
    echo -e "  4. Follow test scenarios from the guide"
    exit 0
elif [ $ERRORS -eq 0 ]; then
    echo -e "${YELLOW}⚠ ${WARNINGS} warning(s) found${NC}"
    echo -e "  Environment is mostly ready, but review warnings above."
    echo ""
    echo -e "You can proceed with testing, but some features may not work optimally."
    echo ""
    echo -e "Next steps:"
    echo -e "  1. Review warnings above"
    echo -e "  2. Review testing guide: ${BLUE}docs/SCREEN_READER_TESTING.md${NC}"
    echo -e "  3. Print checklist: ${BLUE}docs/SCREEN_READER_CHECKLIST.md${NC}"
    exit 0
else
    echo -e "${RED}✗ ${ERRORS} error(s) and ${WARNINGS} warning(s) found${NC}"
    echo -e "  Environment is NOT ready for testing."
    echo ""
    echo -e "Fix the errors above before proceeding with manual testing."
    exit 1
fi
