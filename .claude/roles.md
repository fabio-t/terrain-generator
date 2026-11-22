# Roles in this repository

What each agent in `.claude/agents/` needs to know here. The rules are `AGENTS.md`'s, and this file
only says which of them each role trips over. The agents and `.claude/HARNESS.md` are shared with
the maintainer's other repositories; this file is not.

## planner

A plan is a GitHub issue or a section of the PR description; there is no planning directory. An
option is planned for every entry point at once (rule 4), and anything that moves generated output
is planned as a breaking change (rule 1).

## architect

One core, five entry points: the library in `src/`, the CLI, the GUI in its own workspace under
`gui/`, the NIF in `native/terrain_generator_nif/` and the C ABI in `native/terrain_generator_ffi/`.
Generation stays in the core; an entry point only translates options and results.

## coder

- Never update a golden checksum in `tests/determinism.rs` to get green. If one moves, stop and
  report which layers and options moved.
- `noise` stays pinned at `=0.9.0`.
- Code in `src/` and `native/` builds on the MSRV in `Cargo.toml`; the GUI is exempt.
- The library builds with `--no-default-features` and with `--features png` alone.
- A new generation option reaches the CLI, the GUI, the NIF, the FFI and the README, or the report
  says which it skips.
- Bump no version. Commit lockfile changes with the dependency change.
- A CHANGELOG entry under `## Unreleased`, grouped by entry point, for anything a user can notice.
- Comments say why, never what.

The gate on what you touched: `mise run ci`; `mise run elixir` where `lib/`, `test/` or the NIF
moved; `mise run java` where `java/` or the FFI moved. If generation changed, render a map
(`cargo run --release -- ...`) and give the reviewer its path.

## adversary

Your remit is `AGENTS.md` in full, with determinism first: the same seed and parameters must give
the same map on every platform and from every entry point. Also the MSRV, the feature builds, an
option missing from an entry point, a version bumped outside a release, and `unsafe` in `native/`.

## security

Paths that trigger it: `native/` (the `unsafe` code behind the C ABI and the NIF), `include/`,
`java/`, and `.github/workflows/`. Threats that matter: a pointer, length or size crossing the C
ABI or the NIF used before it is checked; a size from the caller that allocates without bound; a
workflow permission wider than its job needs; a secret (`HEX_API_KEY`, the crates.io publishing)
reaching a job that does not publish. Run `cargo audit` and `mix hex.audit` where they are
installed, and say so where they are not. Every release gets this pass (`AGENTS.md`, "Releasing",
pass 2).

## looker

Maps come from the CLI (`cargo run --release -- --help` for the options) or `mise run gui`. Look
for what passes every test: seams at chunk or tile boundaries, a straight line ruled across noise,
rivers that run uphill or stop short of the sea, lakes with no outlet where breaching was on, an
island mask that shows its shape function.

## runner

The toolchain is pinned in `mise.toml`: run `mise run <task>`, or `cargo +<version>` with the
pinned version, never a newer clippy. The GUI is its own workspace: `--manifest-path
gui/Cargo.toml`. For `cargo test`, every `test result:` line verbatim.
