#!/usr/bin/env bash
# Every crate's unit tests at once, against the committed lock.
#
# One of them compares the screensaver's vendored ASCII art against the copy
# shedos-branding installs, so a machine that does not have that package gets
# the file from the branding repository first. The pipeline's container has no
# ShedOS channel to install the package from, and the comparison is the whole
# point of the test.
set -uo pipefail

here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &> /dev/null && pwd)
repo_root=$(cd -- "$here/../.." &> /dev/null && pwd)

branding=/etc/shedos-ascii.txt
branding_url=https://raw.githubusercontent.com/shed-os/shedos-branding/main/tree/etc/shedos-ascii.txt

if [[ ! -f $branding ]]; then
    echo "$branding is absent — taking it from shedos-branding"
    tmp=$(mktemp)
    trap 'rm -f "$tmp"' EXIT
    if ! curl -fsSL -A 'shedos-ui (+https://shedos.org)' -o "$tmp" "$branding_url"; then
        echo "FATAL: cannot fetch $branding_url" >&2
        exit 1
    fi
    sudo install -Dm644 "$tmp" "$branding" || exit 1
fi

cd "$repo_root" || exit 1
cargo test --locked --workspace
