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
| Desktop GUI    | `gui/`                        | eframe app, `publish = false`              |
| Elixir library | `lib/`, `native/terrain_generator_nif/` | Rustler NIF                       |
| Java library   | `java/`, `native/terrain_generator_ffi/` | C ABI + Foreign Function & Memory API |

Integration tests are in `tests/`, and Elixir tests in `test/`. API docs
per language are in `docs/`.

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
   currently set by rustler. CI checks every crate except the GUI against
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
6. **`Cargo.lock` is committed**, and CI builds with `--locked`.

## Checks

The toolchain is pinned in [`mise.toml`](mise.toml), and CI reads the same
line. With [mise](https://mise.jdx.dev):

```sh
mise install       # the pinned Rust
mise run ci        # fmt, clippy -D warnings, tests, the feature builds
mise run elixir    # if you touched lib/, test/ or the NIF (needs Elixir/OTP)
mise run java      # if you touched java/ or the FFI (needs JDK 22+)
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
- **Releases are cut by pushing a tag** (`v1.2.3`). `release.yml` builds the
  CLI and the native libraries for every platform and publishes the
  release, with that version's CHANGELOG section as the notes.
- **Bumping the toolchain** is a one-line change to `mise.toml`. Run
  `mise run lint` on the new version first: new clippy lints fail CI under
  `-D warnings`, which is how CI broke on Rust 1.98.

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
