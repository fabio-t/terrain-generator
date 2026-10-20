## What and why

<!-- What changes, and what it is for. Link an issue if there is one. -->

## Generated terrain

<!-- Same seed + same parameters must give the same map: users store
     seeds, and CHANGELOG.md treats any change to the output as breaking.
     tests/determinism.rs holds golden checksums that catch it. -->

- [ ] Output unchanged: `cargo test --test determinism` passes untouched
- [ ] Output changes on purpose: golden checksums updated, and the
      CHANGELOG entry says which layers move and for which options

## Checks

What CI runs. It is not triggered by pull requests, so a maintainer
dispatches it; run these locally first.

- [ ] `cargo fmt --all --check`
- [ ] `cargo clippy --workspace --all-features -- -D warnings`
- [ ] `cargo test --workspace`
- [ ] `cargo build --no-default-features`, if you touched features or
      imports (the library must build without the CLI and `png`)
- [ ] `mix format --check-formatted && mix test`, if you touched `lib/`,
      `test/` or `native/terrain_generator_nif/`
- [ ] `./java/build.sh`, if you touched `java/` or
      `native/terrain_generator_ffi/`

## Versions and pins

- [ ] `Cargo.lock` is committed alongside any dependency change (CI builds
      with `--locked`)
- [ ] The libraries still build on the MSRV in `Cargo.toml`
      (`rust-version`). If you need a newer Rust, bump it and say why; the
      GUI is exempt
- [ ] `noise` is untouched, or the golden checksums above still pass:
      its output is part of ours
- [ ] Not bumping versions here: releases do that, in `Cargo.toml`, both
      `native/*/Cargo.toml`, `mix.exs`, and the jar name in the docs

## Docs

- [ ] `CHANGELOG.md` has an entry under `## Unreleased`, if users can
      notice the change
- [ ] A new or changed option is in the README, and reaches the CLI, the
      GUI and both bindings, or the PR says which it skips
