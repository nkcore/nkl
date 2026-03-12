# Agent Rules

## Build System

Use `cargo` for builds and tests. Add dependencies with `cargo add <crate>`.

## No Emojis

Do not use emojis in this repository, including code, comments, output, and documentation.

## Project Development

Use the `/pma` three-phase workflow:

1. **Investigation** -- inspect the current state, trace call chains, and check `docs/changelog.md`
2. **Proposal** -- write the proposed approach and wait for approval; create `docs/plan/PLAN-NNN.md` for non-trivial work
3. **Implement -> Verify -> Record** -- implement after approval, verify the result, and update the changelog

Documentation locations:

- `README.md` -- usage guide
- `docs/task/index.md` -- task index
- `docs/plan/index.md` -- plan index
- `docs/architecture.md` -- architecture document
- `docs/changelog.md` -- changelog
