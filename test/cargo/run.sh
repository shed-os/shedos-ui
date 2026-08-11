#!/usr/bin/env bash
# Every crate's unit tests at once, against the committed lock.
set -uo pipefail

here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &> /dev/null && pwd)
repo_root=$(cd -- "$here/../.." &> /dev/null && pwd)

cd "$repo_root" || exit 1
cargo test --locked --workspace
