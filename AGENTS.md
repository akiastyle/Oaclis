# Oaclis Agent Notes

## Project Scope
- Local project root: `/root/Oaclis`.
- Oaclis is the active project unless the user explicitly says otherwise.
- Work one agreed step at a time. A request to discuss or inspect does not authorize implementation, installation, deployment, or synchronization.
- The UI currently runs with `OACLIS_PAUSE_SETTINGS=0 OACLIS_PAUSE_CHECKS=1 OACLIS_PAUSE_TRANSFERS=1`. Keep actual chat/file/app transfers and automatic checks paused until the user explicitly asks to resume them. Shared presentation, positions, links and saved source inventories continue replicating independently of that pause.

## Core Rule
- Verify before changing.
- Do not assume a file, DB function, query, wrapper, uploader, service, or scheduled task is unused.
- Changes to actual Codex archives, peer project files, authentication, SSH keys, or services require an explicit instruction covering the action.
- Authorization already given in the session remains valid; do not ask again for the same action. Stay within its scope.
- Never delete or overwrite user files, chat archives, configuration helpers, or DB rows because they look obsolete.

## Backend Structure
- Rust entrypoint: `src/main.rs`; HTTP adapter: `src/web.rs`. Catalog reads live in `src/catalog.rs`, initial/manual inventory detection in `src/initial_inventory.rs`, transfer execution in `src/transfer_job.rs`.
- Keep chat handling, paths, transfers, rules, preparation, settings, and presentation in small independent modules.
- Python helpers live in `tools/`; frontend assets live in `ui/`. `ui/app.js` owns shared state, API helpers and startup/polling; `ui/map.js` owns map/links/library, `ui/panels.js` owns contextual panels, and `ui/console.js` owns chat consoles. These classic deferred scripts share controller globals; keep their loading order in `ui/index.html` and their Rust asset routes consistent.
- `ui/badge-motion.js` and `ui/badge-motion.css` handle animation only. They do not decide or execute transfers, calculate progress, or access storage.
- Public HTTP access stays routed through the Rust adapter. Do not expose internal helpers, credentials, or configuration files directly.
- Prefer existing reliable Linux tools for transport: SSH and rsync. Oaclis owns scheduling; do not add cron jobs or systemd scheduling as a side effect.

## Database
- Local settings DB: `/root/.config/oaclis/appearance.sqlite`; other devices use their own user configuration directory.
- `appearance` holds presentation in `value`, visibility in `hidden`, and module switches and source data in `sync`.
- Keep source data separate by stable device identity, and chats identified by their stable Codex ID.
- Update only the fields belonging to an operation: checks must not overwrite user labels, icons, colors, visibility, switches, or another source's data.
- Preserve schema, data, routines where applicable, triggers, keys, indexes, constraints, and identity values during migrations. Do not treat copying tables alone as a complete migration.
- A partial migration requires an agreed scope for omitted objects. Test migrations on isolated copies and discard those copies after the check.
- Completed checks remain saved. Do not expire or automatically repeat them on refresh or panel opening.
- Each chat/device link has a persisted `flow` and a unique generation. Only its designated executor compares or executes it. Initial alignment runs once; completed one-way links stop; bidirectional links compare every 60 seconds by default and transfer only after Play. Compare content proofs, never infer the winner from clocks. Conflicts block writes.
- Refresh updates inventories only. Keep chat content monitoring separate from AI path/app detection; a new message must not restart completed inventory analysis.
- Read-only status/catalog requests must not create tables or modify data.

## Peer Access and Deployment
- Use the configured SSH aliases and approved pairing. Keep identities and access details in their existing configuration locations.
- Do not print credentials, commit them, or copy them into `AGENTS.md` or commands shown to the user.
- Deploy only explicitly authorized files or builds, to explicitly authorized devices.
- Compare the relevant source and destination before deployment; afterward verify the target and run the smallest relevant check.
- Do not use `rsync --relative` with absolute paths unless the resulting destination layout is explicitly intended and verified.
- An unreachable device remains paused. Do not attempt transfers or remote test actions at every development step.
- Reconnection follows the saved automatic/manual rule; it does not authorize repeating completed analysis.
- Do not publish local changes, push Git commits, or update clients unless the user requests that action.

## Background Services
- Do not start, stop, remove, or recreate services/timers unless explicitly requested or required by an already authorized operation.
- Opening the UI does not authorize synchronization, installations, or new environment checks.
- Simulations must be isolated from real transfers and databases. Remove temporary demos when the user asks.

## Working Style
- Keep changes small and targeted.
- Prefer inspection and a short explanation before modifying sensitive areas.
- Do not create backup clutter in WSL/test or in the project unless the user asks for it.
- Do not revert unrelated user changes.
- Keep edits ASCII unless the file already uses Unicode or the project requires translated text.
- Preserve inherited UI preferences from `/root/AGENTS.md` and the design already agreed with the user.
- Reuse components and CSS variables. Keep animation instances and DOM elements stable across unchanged updates.
- Run relevant checks, then stop. Avoid repeated verification without a new change, failure, or unresolved concern.

## File Versioning
- Every maintained, nonempty application source file has its own `filename vX.Y.Z` header. This is separate from the application version, Git, and asset cache URLs.
- JavaScript and Rust use `// filename vX.Y.Z`; Python, shell, and PowerShell use `# filename vX.Y.Z` after any required shebang/encoding declaration. PHP places `// filename vX.Y.Z` after `<?php`; CSS uses `/* filename vX.0.Z */`; HTML uses `<!-- filename vX.0.Z -->`.
- The project-wide baseline authorized on 2026-10-14 uses `X=1+floor(Z/1000)`, `Y` equal to the current named functions/methods, and `Z` equal to the current physical lines excluding the version header. CSS and HTML use `Y=0`.
- Count declarations, class/object methods, and functions assigned to named variables; do not count anonymous callbacks. Preserve an existing version as the baseline. Outside this authorized alignment, an unversioned file starts at `v1.0.0` unless the user specifies otherwise.
- `Y` increases by one for each named function or method added or removed; changing its body alone does not increment `Y`. CSS and HTML always have `Y=0`.
- `Z` increases by added lines plus removed lines. Replacing one line counts as two. Exclude only the version header from this comparison.
- `X` increases by one for each new multiple of 1000 crossed by `Z`; do not reset `Z`. Example: `v1.25.990` plus 30 changed lines becomes `v2.25.1020`.
- These are historical counters: never recount the current file to replace an existing version, and never decrease counters when code is deleted.
- When extracting a module, inherit the original file's X/Y/Z baseline, then add the extracted functions and new file lines. The original file counts those functions and lines as removals. Do not restart an extracted module at v1.0.0.
- Compare against the verified original before editing and update only versions of files actually changed. Never add or change cache parameters as part of header versioning.
- Exclude empty placeholder files, third-party/vendor code, generated/minified assets, mail templates where present, and non-application documents or binary assets.
- Adding these rules does not authorize a bulk rewrite of existing file headers. Initial project-wide alignment requires a separate user instruction.

## General Application Version
- Calculate the displayed application version directly from maintained source headers at compilation, without a fixed baseline or offset.
- X is the arithmetic mean of all included X values; Y is the mean of only positive Y values; Z is the mean of all included Z values, including zeros.
- Round each mean to the nearest integer, with halves rounded up. Display vX.Y.Z in small absolute-positioned text at the bottom right of the page.
- build.rs includes versioned source files in src, ui, tools, scripts, tests, and the root bootstrap/build scripts. File Versioning exclusions still apply; documents and binary assets are excluded.
- This calculated display does not change Cargo package versions, individual file counters, or asset cache URLs.

## Presentation Deletion
- Deleting a connection removes only that chat/device association; never alter chat visibility, appearance, inventories, or user files.
- Removing a chat from the list sets hidden and removes its associations, while retaining its label, custom icon, color, analysis and inventories. Do not use a removed flag for chat presentation.
- Metadata replication must not clear chat appearance when an old deletion tombstone is received.

## Device Version Alignment
- Compare source fingerprints rather than numerical application versions: the displayed means can coincide across different source builds.
- Remote app updates are explicit user actions. Preserve configuration and user data; refuse dirty or divergent checkouts, use fast-forward Git updates, then compile and restart.
- Include the committed revision in release builds by compiling after the commit before deployment.
