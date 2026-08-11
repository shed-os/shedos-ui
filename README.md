# shedos-ui

The surfaces ShedOS puts on the screen: the login greeter, the screensaver and
lock screen, the Alt-Tab switcher, the power menu and the first-run tour. Six
Rust crates in one workspace, five of them packaged; the sixth,
`shedos-prompt-ui`, is the library the others draw with — the font atlas, the
clock, the password prompt, the power glyphs.

They live together because they share that library and a dependency graph, and
splitting them would mean five copies of the same lock drifting apart. One
`Cargo.lock` at the root pins the set for all of them.

## Layout

Each package keeps its own directory, and that directory is both the workspace
member and where its PKGBUILD sits:

```
shedos-prompt-ui/     the shared renderer, no package
shedos-greeter/       greetd greeter
shedos-power/         power-menu overlay
shedos-switcher/      Alt-Tab switcher
shedos-tour/          first-run welcome tour
shedos-screensaver/   screensaver and lock screen, eight crates under crates/
```

The screensaver is the one that does not fit the pattern: it is eight crates,
so its code is under `crates/` and its directory holds only the packaging.

## Building

The PKGBUILDs do not build the tree beside them. Each fetches this repository
at the tag its `pkgver` names and builds out of that checkout, so what a
package contains is decided by the tag rather than by whatever is in the
working directory:

```
cd shedos-greeter && makepkg
```

Cutting a release therefore means tagging the commit the PKGBUILDs should be
built from. The five `pkgver`s move together and one tag covers all of them.

For everyday work, `cargo build` and `cargo test --workspace` from anywhere in
the tree do what you expect; the binaries land in the workspace root's
`target/`.

## CI

`.github/workflows/ci.yml` calls the shared pipeline in `shed-os/shedos-ci`,
which builds all five packages, runs every `test/*/run.sh`, and asks
`shed-os/shedos-release` to publish what it built. The suites are
`test/cargo` (the workspace's unit tests) and `test/screensaver` (the
screensaver binary's read-only modes).

One of the unit tests compares the screensaver's vendored ASCII art against
the copy `shedos-branding` installs at `/etc/shedos-ascii.txt`. On a machine
without that package — the pipeline's container, for one — `test/cargo/run.sh`
fetches the file from the branding repository first, because a comparison that
quietly does not happen is how the two drift apart.

These packages are also still built out of the ShedOS monolith. Until that
stops, a change made here has to be made there too.
