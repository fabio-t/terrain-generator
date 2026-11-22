# Roles in this repository

What each agent in `.claude/agents/` needs to know here. The rules are `AGENTS.md`'s, and this file
only says which of them each role trips over. The agents and `.claude/HARNESS.md` are shared with
the maintainer's other repositories; this file is not.

## planner

AGENTS.md says where plans live nowhere, so a plan comes back in your reply, for the lead to put
in the PR description or an issue; write no file unless the dispatch names one. Plan an option for
every entry point at once (rule 4), and anything that moves generated output as a breaking change
(rule 1).

## architect

One core, five entry points (AGENTS.md, "What this is"). Generation stays in the core; an entry
point only translates options and results.

## coder

Rules 1 to 6 of `AGENTS.md` bind you as written, and rule 1 first: if a golden checksum moves,
stop and report which layers and options, and never update it to get green. Change `Cargo.lock`
with `Cargo.toml` in the same diff. Add a CHANGELOG entry where a user can notice the change.

The gate on what you touched is "Checks" in `AGENTS.md`: `mise run ci`; `mise run elixir` where
`lib/`, `test/` or the NIF moved; `mise run java` where `java/` or the FFI moved. If generation
changed, render a map (`cargo run --release -- ...`) and give the reviewer its path.

## adversary

Your remit is `AGENTS.md`'s rules in full, with determinism (rule 1) first. Also the MSRV, the
feature builds, an option missing from an entry point, a version bumped outside a release, and
`unsafe` in `native/`.

## security

Paths that trigger it, and the only list of them: `native/` (the `unsafe` code behind the C ABI and
the NIF), `include/` (the C header), `java/`, and anything under `.github/`. Threats that matter: a pointer, length or size crossing the C
ABI or the NIF used before it is checked; a size from the caller that allocates without bound; a
workflow permission wider than its job needs; a secret (`HEX_API_KEY`) reaching a job that does not publish, or `id-token: write` (crates.io
trusted publishing) granted to one that does not. Run `cargo audit` and `mix hex.audit` where they are
installed, and say so where they are not. Every release gets this pass (`AGENTS.md`, "Releasing",
pass 2).

## looker

Maps come from the CLI (`cargo run --release -- --help` for the options) or `mise run gui`. Look
for what passes every test: a straight line ruled across noise, rivers that run uphill or stop
short of the sea, an island mask that shows its shape function, erosion that leaves visible
banding. Small lakes without an outlet are by design (basin breaching only opens basins above its
size fraction).

## runner

The toolchain is pinned in `mise.toml`: run `mise run <task>`, or `cargo +<version>` with the
pinned version, never a newer clippy. The GUI is its own workspace: `--manifest-path
gui/Cargo.toml`. For `cargo test`, every `test result:` line verbatim.
