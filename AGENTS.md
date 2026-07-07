# AGENTS.md

## Project overview

This repository contains `saga-seeker-html2md`, a Windows-oriented Rust CLI tool for converting Saga & Seeker HTML character sheets into Markdown.

The primary usage is double-click execution of the built `.exe`, not command-line operation.

## Core behavior to preserve

Do not change these existing behaviors:

- Read `.html` and `.htm` files from `input/`.
- Write Markdown files to `output/`.
- Use the executable's directory as the base directory.
- Keep `input` / `output` / `log.txt` as the runtime paths.
- Extract character status fields.
- Extract body sections.
- Extract skill names.
- Extract skill details.
- Preserve custom skills where `type == ""`.
- Keep excluding `魅力`.
- Keep detailed results in `log.txt`.
- Keep normal console output minimal:
  - `処理開始`
  - completion message
  - `Enterキーで終了します...`

## Do not do

- Do not introduce an `outputs` runtime directory.
- Do not require command-line arguments for normal use.
- Do not recommend Defender exclusions or Task Scheduler as normal usage.
- Do not change the Markdown output format unless required to fix a bug.
- Do not remove status, body section, skill name, or skill detail output.
- Do not commit build artifacts unless explicitly asked.
- Do not commit private test character sheets to a public release.

## Required checks

Run these before finalizing changes:

```powershell
cargo fmt --check
cargo check
cargo build --release
```

Then test conversion:

1. Copy `samples/sample.html` into `input/sample.html`.
2. Run the built executable.
3. Compare `output/sample.md` with `samples/expected.md`.
4. Test private fixture zips if available.
   - Do not commit private fixture zips.
   - Do not commit real character sheets or generated Markdown outputs.

## Difference policy

Allowed differences:

- Console output.
- `log.txt` contents.
- Timing values.
- Version strings.

Treat these as regressions unless explicitly approved:

- Status output differences.
- Body section differences.
- Skill name differences.
- Skill detail differences.
- Missing custom skills where `type == ""`.
- Reappearance of `魅力`.
