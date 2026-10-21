# Terrain generator: the brief for agents

This file is for any coding agent or assistant working in this repository,
whatever tool runs it, and for people who want the short version. It holds
what you can't work out from the code. What the project does is in
[`README.md`](README.md), and what changed when is in
[`CHANGELOG.md`](CHANGELOG.md).

## What this is

A deterministic procedural terrain generator in Rust. It produces a noise
heightmap, an island or archipelago mask, hydraulic erosion, optional basin
breaching, then hydrology: rivers, lakes and ocean. One core, five ways in:

| Entry point    | Where                         | Notes                                      |
|----------------|-------------------------------|--------------------------------------------|
| Rust library   | `src/` (`lib.rs`)             | Published to crates.io                     |
| CLI            | `src/main.rs`                 | `cli` feature, on by default               |
| Desktop GUI    | `gui/`                        | eframe app, its own workspace, not published |
| Elixir library | `lib/`, `native/terrain_generator_nif/` | Rustler NIF                       |
| Java library   | `java/`, `native/terrain_generator_ffi/` | C ABI + Foreign Function & Memory API |

Integration tests are in `tests/`, and Elixir tests in `test/`. API docs
per language are in `docs/`.

The GUI is a separate Cargo workspace with its own `gui/Cargo.lock`, so
the libraries never resolve eframe and the Hex package ships no GUI code.
Cargo commands run from the root do not reach it: use
`--manifest-path gui/Cargo.toml`, as `mise.toml`'s tasks do.

## Rules

1. **Generated terrain is a public contract.** Users store seeds, so the
   same seed and parameters must give the same map, on every platform and
   from every entry point. `tests/determinism.rs` holds golden checksums.
   If a change moves them, it is a breaking change. Make it on purpose:
   update the checksums, and call it out in the CHANGELOG with which layers
   move and for which options. Never update a checksum just to get green.
   `noise` generates part of the output, so it is pinned exactly
   (`=0.9.0`) and an upgrade is held to the same bar: move the pin by hand,
   run the goldens, and say so in the CHANGELOG.
2. **The libraries keep an MSRV.** `rust-version` in `Cargo.toml` is
   currently set by rustler. CI checks the libraries' workspace against
   it. Code in `src/` and `native/` must build on that version. The GUI is
   an app built with the pinned toolchain and is exempt.
3. **The library builds without its defaults.** `cli` (clap) and `png` are
   optional. CI builds `--no-default-features`, and `--features png` alone,
   so an import that only compiles with them on fails there.
4. **One option, every entry point.** A generation option added to
   `TerrainBuilder` should reach the CLI, the GUI, the NIF and the FFI, plus
   the README's option section. If a PR leaves one out, it says so.
5. **Versions move together, at release time.** The version is in
   `Cargo.toml`, both `native/*/Cargo.toml`, `mix.exs`, and the jar name in
   `README.md` and `docs/java_api.md`. A feature PR bumps none of them.
6. **The lockfiles are committed** (`Cargo.lock`, `gui/Cargo.lock`,
   `mix.lock`), and CI builds with `--locked` (`mix deps.get
   --check-locked`).

## Checks

The toolchain is pinned in [`mise.toml`](mise.toml), and CI reads the same
line. With [mise](https://mise.jdx.dev):

```sh
mise install       # the pinned Rust, Erlang/OTP, Elixir and JDK
mise run ci        # fmt, clippy -D warnings, tests, the feature builds
mise run elixir    # if you touched lib/, test/ or the NIF
mise run java      # if you touched java/ or the FFI
```

Without mise, the commands are in `mise.toml`'s tasks. Run them with the
pinned version (`rustup toolchain install <version> -c clippy -c rustfmt`,
then `cargo +<version> ...`). A newer clippy can flag lints CI's doesn't
know yet, and an older one can miss lints CI enforces.

Green tests do not show the terrain looks right. If you changed generation,
render a map (`cargo run --release -- --help`, or `mise run gui`) and look
at it.

## CI

- **CI does not run on pull requests.** It runs on pushes to `main` and on
  demand (Actions → CI → Run workflow, on any branch). A maintainer
  dispatches it for outside PRs, so the local checks are the only feedback
  before review.
- **Only `main` saves the Actions cache.** Branches and tags restore
  `main`'s. The reasons are in the comments in `.github/workflows/ci.yml`.
  Keep them true if you edit the workflows.
- **Bumping the toolchain** is a one-line change to `mise.toml`. Run
  `mise run lint` on the new version first: new clippy lints fail CI under
  `-D warnings`, which is how CI broke on Rust 1.98.

## Merging

- **Pull requests are squash-merged, always.** One PR becomes one commit
  on `main`, so `main` lists changes rather than the steps of the work.
  Never a merge commit, never a rebase merge, whatever the PR's own
  commits look like.
- **The PR description is the commit message.** The squash commit is
  built from the PR's title and description, and the branch's commits do
  not survive. Write the description the way a commit says why, and keep
  it current as the PR changes.
- **`main`'s history is never rewritten.** No force-push to `main`
  without the maintainer's explicit go-ahead, whatever the reason.

## Releasing

A tag publishes for good: a version on crates.io or Hex can be yanked or
retired, never replaced, and users store seeds against it. So nothing is
tagged until the release PR, the one that moves the version (rule 5), has
had four passes, and its description records what each found:

1. **Dependencies.** Everything is on its latest release, or the PR says
   why not: the Cargo lockfiles (`cargo update --dry-run --verbose` lists
   what is held back), `mix.lock` (`mix hex.outdated`), the GitHub
   Actions, the pinned runtimes in `mise.toml`. The libraries still build
   on the MSRV.
2. **Security.** No known advisory in the Cargo lockfiles (`cargo
   audit`), and no retired Hex package (`mix hex.audit`). The `unsafe`
   code in `native/` is re-read: every pointer, length and size crossing
   the C ABI and the NIF is checked before use. Workflow permissions are
   the least each job needs, and no secret reaches a job that does not
   publish.
3. **Performance.** Generation is timed against the previous release, on
   the same machine, for the README's example maps (`cargo run --release`
   on both tags). A slowdown is explained in the PR, or fixed.
4. **Documentation.** The README, `docs/`, the rustdoc
   (`RUSTDOCFLAGS="-D warnings" cargo doc --no-deps`) and the Elixir docs
   (`mix docs`) match the code. Every option is documented (rule 4). The
   CHANGELOG section reads as release notes, since it becomes them. The
   packages ship what they should (`cargo package --list`,
   `mix hex.build --unpack`).

Then the release PR is squash-merged, and `release.yml` is run on `main`
with `cut` (Actions → Release → Run workflow). It checks that the
manifests agree and the CHANGELOG names the version, builds and tests the
CLI and the native libraries for every platform, and only then tags that
commit (`v1.2.3`, annotated) and publishes the GitHub release, with that
version's CHANGELOG section as the notes. After that it publishes to
crates.io (trusted publishing) and Hex (the `HEX_API_KEY` secret). A
cloud agent session cannot push tags, which is why the workflow makes
the tag. A hand-pushed tag releases the same way, and is how a
pre-release (`v1.2.3-rc1`) is made; it stops at the GitHub release. If a
release fails after its tag exists, run `release.yml` on the tag to
finish it: every step skips what is already published.

## Writing

- **Commit messages say why.** The subject says what the change is for. The
  body gives the reasoning, and the measurements behind it when there are
  any. Read `git log` for the house style. One concern per commit.
- **Comments explain why, not what.** They cover the decision, the
  trade-off, the number that justified it. When a comment is wrong it is a
  bug, so fix it in the same change.
- **CHANGELOG entries** go under `## Unreleased`, grouped by entry point,
  and are written for users, not for reviewers.

## Agent-specific files

This file is the brief. Tool-specific instruction files (`GEMINI.md`,
`.cursorrules`, `.windsurfrules`, and the like) are git-ignored, so the
rules live in one place. Point your tool at this file instead.
`CLAUDE.md` is the one exception: Claude Code reads only that name, and it
contains nothing but an import of this file.
