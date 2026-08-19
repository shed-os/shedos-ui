#!/usr/bin/env bash
# Read-only-mode tests for shedos-screensaver: hermetic, no live terminal
# mucking beyond T13's pty smoke via script(1).

set -uo pipefail

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &> /dev/null && pwd)
REPO_ROOT=$(cd -- "$SCRIPT_DIR/../.." &> /dev/null && pwd)

# Build from HEAD when cargo is available — preferring a stale
# prebuilt binary silently tested week-old code. The prebuilt paths
# stay as fallback for environments without a Rust toolchain.
if command -v cargo >/dev/null 2>&1; then
    (cd "$REPO_ROOT/shedos-screensaver" \
        && cargo build --release --quiet -p 'shedos-screensaver-*') \
        || echo "WARN: cargo build failed; falling back to a prebuilt binary" >&2
fi

BIN=
for candidate in \
    "$REPO_ROOT/target/release/shedos-screensaver" \
    "$REPO_ROOT/target/debug/shedos-screensaver"; do
    if [[ -x $candidate ]]; then
        BIN=$candidate
        break
    fi
done

if [[ -z $BIN ]]; then
    echo "FATAL: no shedos-screensaver binary found." >&2
    echo "Run \`cd shedos-screensaver && cargo build\` first." >&2
    exit 1
fi

PASSED=0
FAILED=0
FAILURES=()

ok() {
    PASSED=$((PASSED + 1))
    printf "  \e[32m✓\e[0m %s\n" "$1"
}
fail() {
    FAILED=$((FAILED + 1))
    FAILURES+=("$1")
    printf "  \e[31m✗\e[0m %s\n" "$1"
    if [[ -n ${2:-} ]]; then
        printf "      %s\n" "$2"
    fi
}
expect_exit() {
    local name=$1 expected=$2 actual=$3 out=$4
    if [[ $actual -eq $expected ]]; then
        ok "$name"; return 0
    fi
    fail "$name" "expected exit $expected, got $actual; output: $out"
    return 1
}
expect_contains() {
    local name=$1 needle=$2 haystack=$3
    if [[ $haystack == *"$needle"* ]]; then
        ok "$name"; return 0
    fi
    fail "$name" "expected to contain '$needle'; got: $(printf '%s' "$haystack" | head -c 200)"
    return 1
}

echo "Testing $BIN"
echo

# T1: --help-summary
out=$("$BIN" --help-summary 2>&1); code=$?
expect_exit "T1 --help-summary exits 0" 0 "$code" "$out"
expect_contains "T1 --help-summary mentions SHEDOS" "SHEDOS" "$out"

# T2: --help
out=$("$BIN" --help 2>&1); code=$?
expect_exit "T2 --help exits 0" 0 "$code" "$out"
expect_contains "T2 --help mentions effect" "effect" "$out"

# T3: --list (effects)
out=$("$BIN" --list 2>&1); code=$?
expect_exit "T3 --list exits 0" 0 "$code" "$out"
for effect in rain decrypt print scattered wipe slide expand crumble spotlights burn colorshift glitch quantum synthgrid matrix-rain hologram \
              neon-trace blackhole shockwave liquid-fill constellation interlace thermal data-stream tetris boot-sequence; do
    expect_contains "T3 --list contains $effect" "$effect" "$out"
done

# T4: --list-logos
out=$("$BIN" --list-logos 2>&1); code=$?
expect_exit "T4 --list-logos exits 0" 0 "$code" "$out"
for variant in block slim ansi-shadow big boxed gradient checker shadow-cast mirror-flip; do
    expect_contains "T4 --list-logos contains $variant" "$variant" "$out"
done

# T5: --help-effect rain
out=$("$BIN" --help-effect rain 2>&1); code=$?
expect_exit "T5 --help-effect rain exits 0" 0 "$code" "$out"
expect_contains "T5 --help-effect rain mentions title" "Rain" "$out"

# T6: --complete-bash
out=$("$BIN" --complete-bash 2>&1); code=$?
expect_exit "T6 --complete-bash exits 0" 0 "$code" "$out"
[[ -n "$out" ]] && ok "T6 --complete-bash output non-empty" || fail "T6 output empty"

# T7: --complete-fish
out=$("$BIN" --complete-fish 2>&1); code=$?
expect_exit "T7 --complete-fish exits 0" 0 "$code" "$out"
expect_contains "T7 --complete-fish uses fish format" "complete -c shedos-screensaver" "$out"

# T8: --complete-zsh
out=$("$BIN" --complete-zsh 2>&1); code=$?
expect_exit "T8 --complete-zsh exits 0" 0 "$code" "$out"
[[ -n "$out" ]] && ok "T8 --complete-zsh output non-empty" || fail "T8 output empty"

# T9: --effect nonsense → exit 2
out=$("$BIN" --effect nonsense 2>&1); code=$?
expect_exit "T9 --effect nonsense exits 2" 2 "$code" "$out"
expect_contains "T9 stderr names the bad effect" "nonsense" "$out"

# T10: --logo nonsense → exit 2
out=$("$BIN" --logo nonsense 2>&1); code=$?
expect_exit "T10 --logo nonsense exits 2" 2 "$code" "$out"
expect_contains "T10 stderr names the bad logo" "nonsense" "$out"

# T11: --color rubbish → exit 2
out=$("$BIN" --color rubbish 2>&1); code=$?
expect_exit "T11 --color rubbish exits 2" 2 "$code" "$out"
expect_contains "T11 stderr names the bad color" "rubbish" "$out"

# T12: the verb shim's flag vocabulary against the binary's own
#
# `shedman screensaver` reaches the binary through a shim, and the shim
# answers the completion contract itself because the dispatcher's completers
# want a word list where the binary emits a completion script. That leaves two
# lists, and this is what keeps them one: every long option clap knows is one
# the shim offers, and every one the shim offers is one clap knows.
SHIM=$REPO_ROOT/shedos-screensaver/tree/usr/libexec/shedman/screensaver
if [[ -x $SHIM ]]; then
    clap_opts=$("$BIN" --complete-bash 2>/dev/null \
        | sed -n 's/^ *opts="\(.*\)"$/\1/p' | tr ' ' '\n' \
        | grep '^--' | grep -vE '^--(help-summary|complete-(bash|zsh|fish))$' \
        | LC_ALL=C sort -u)
    shim_opts=$("$SHIM" --complete-bash 2>/dev/null \
        | grep '^--' | LC_ALL=C sort -u)
    if [[ -z $clap_opts ]]; then
        fail "T12 the binary's completion script names its options"
    elif drift=$(LC_ALL=C comm -3 <(printf '%s\n' "$clap_opts") \
            <(printf '%s\n' "$shim_opts")) && [[ -z $drift ]]; then
        ok "T12 the shim offers exactly the options the binary takes"
    else
        fail "T12 the shim and the binary disagree about the options" \
            "$(tr '\n' ' ' <<<"$drift")"
    fi
else
    fail "T12 the verb shim is executable" "$SHIM"
fi

# T13: pty smoke for one effect cycle
if command -v script >/dev/null 2>&1; then
    if script -q -c "$BIN --mode=tty --effect rain --logo block --duration 0.5 --hold 0" /dev/null > /dev/null 2>&1; then
        ok "T13 pty rain on block --duration 0.5 exits cleanly"
    else
        fail "T13 pty cycle returned non-zero"
    fi
else
    echo "  (skipping T13: script(1) not installed)"
fi

echo
echo "Passed: $PASSED"
echo "Failed: $FAILED"
if [[ $FAILED -gt 0 ]]; then
    echo "Failures:"
    for f in "${FAILURES[@]}"; do
        echo "  - $f"
    done
    exit 1
fi
exit 0
