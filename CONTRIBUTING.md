# Contributing

Thanks for helping. Bug reports and ideas are welcome as GitHub issues; pull requests too.

- Keep the hard rules from `CLAUDE.md`: recording is opt-in per call, transcription stays on the
  device, and only text is kept.
- Before a pull request, run the checks: `cargo clippy`, `cargo test`,
  `cargo check --target x86_64-pc-windows-msvc --no-default-features` and `scripts/check-mod.sh`.
- Never attach real call data (transcripts, chat, screenshots of real meetings) to an issue or
  a test. Use made-up people and companies.

**License of contributions.** Echo is under the [Functional Source License](LICENSE.md). By
submitting a contribution you agree that it is licensed under the same terms, and you grant the
project's licensor (Artyom Rozumenko) a perpetual, irrevocable, worldwide, royalty-free license
to use, modify and relicense your contribution, including under other license terms. You confirm
that you have the right to grant this (for example, your employer allows it).
