# Oaclis Agent Notes

## Project Scope
- Local project root: `/root/Oaclis`.
- Oaclis is the active project unless the user explicitly says otherwise.
- Work one agreed step at a time. A request to discuss or inspect does not authorize implementation, installation, deployment, or synchronization.
- Normal operation has settings exchange, local checks and link matching enabled. Do not introduce development pauses unless the user asks. Transfers always require Play, including the first transfer.

- The user explicitly chose to keep the ROG master running as root. Do not migrate its Codex archives, project paths or tools to kim. Bootstrap and the restricted SSH gateway must support the configured account, including root.

## Core Rule
- Verify before changing.
- Do not assume a file, DB function, query, wrapper, uploader, service, or scheduled task is unused.
- Changes to actual Codex archives, peer project files, authentication, SSH keys, or services require an explicit instruction covering the action.
- Authorization already given in the session remains valid; do not ask again for the same action. Stay within its scope.
- Never delete or overwrite user files, chat archives, configuration helpers, or DB rows because they look obsolete.

## Backend Structure
- Rust entrypoint: `src/main.rs`; HTTP adapter: `src/web.rs`. Catalog reads live in `src/catalog.rs`, initial/manual inventory detection in `src/initial_inventory.rs`, transfer execution in `src/transfer_job.rs`.
- Keep chat handling, paths, transfers, rules, preparation, settings, and presentation in small independent modules.
- Operational application logic is Rust-only in `src/`; no Python helper is required. Project Python files remain read-only inventory evidence. Frontend assets live in `ui/`. `ui/app.js` owns shared state, API helpers and startup/polling; `ui/map.js` owns map/links/library, `ui/panels.js` owns contextual panels, and `ui/console.js` owns chat consoles. These classic deferred scripts share controller globals; keep their loading order in `ui/index.html` and their Rust asset routes consistent.
- `ui/locale.js` is the single EN/IT settings, translation and voice-command source for native UI and mobile PWA. It follows browser preferences by default; source settings can pin a language. Publish this same file to the relay; do not create a separate mobile dictionary. Translate interface copy only, never user labels, chat content or terminal output.
- `ui/icons.js` is the shared icon catalog; the relay uses the same published source.
- `ui/badge-motion.js` and `ui/badge-motion.css` handle animation only. They do not decide or execute transfers, calculate progress, or access storage.
- The OVH-hosted mobile relay remains PHP. Do not upload or execute Rust CGI, background-process or proxy capability probes on this shared hosting: a previous probe triggered OVH security suspension. Native Rust runs only locally and on the paired devices.
- Mobile console requests require a phone signature checked by the master against locally pinned keys and local chat grants. Never execute relay-supplied `value` commands or fall back to unsigned requests. The QR pairing secret stays off the relay; `mobile-relay.json` is owned by the in-memory mobile authorization store and is never shared with peers or the browser.
- Public HTTP access stays routed through the Rust adapter. Do not expose internal helpers, credentials, or configuration files directly.
- Prefer existing reliable Linux tools for transport: SSH and rsync. Oaclis owns scheduling; do not add cron jobs or systemd scheduling as a side effect.

## Database
- Operational state is owned by Rust in memory. Persist one JSON per former table in the configuration directory: `appearance.json`, `analysis.json`, `chat_links.json`, `transfers.json`. Keep these existing keys and fields; do not invent a new storage model. SQLite is read only for the initial legacy import and the external Codex title catalog.
- `appearance` holds presentation in `value`, visibility in `hidden`, and module switches and source data in `sync`.
- Keep source data separate by stable device identity, and chats identified by their stable Codex ID.
- Update only the fields belonging to an operation: checks must not overwrite user labels, icons, colors, visibility, switches, or another source's data.
- Preserve schema, data, routines where applicable, triggers, keys, indexes, constraints, and identity values during migrations. Do not treat copying tables alone as a complete migration.
- A partial migration requires an agreed scope for omitted objects. Test migrations on isolated copies and discard those copies after the check.
- Completed checks remain saved. Do not expire or automatically repeat them on refresh or panel opening.
- Each chat/device link has a persisted `flow` and a unique generation. Only its designated executor compares or executes it. Completed one-way links stop; bidirectional links compare every 60 seconds by default. Every transfer requires Play. The newer chat determines the direction of all enabled modules; compare content proofs and, for divergent histories, recorded UTC chat activity, never filesystem copy times. An unresolved tie blocks writes.
- Refresh updates inventories only. Keep chat content monitoring separate from AI path/app detection; a new message must not restart completed inventory analysis.
- Read-only status/catalog requests must not rewrite JSON data. All mutations use `state_store`; do not reintroduce Python storage helpers, per-request subprocesses, independent writers or new databases. Native peer requests use `state_transport` over the existing approved SSH channel.

## Peer Access and Deployment
- The mobile device named `test` is intentionally permanent. Reuse its private browser profile and virtual authenticator in `/root/.local/share/oaclis/mobile-test` via the ignored local test `tools/test_mobile_device.cjs`; do not remove this pairing during routine test cleanup unless the user requests it. Never commit its credentials or browser profile.
- Use the configured SSH aliases and approved pairing. Keep identities and access details in their existing configuration locations.
- Do not print credentials, commit them, or copy them into `AGENTS.md` or commands shown to the user.
- Deploy only explicitly authorized files or builds, to explicitly authorized devices.
- Compare the relevant source and destination before deployment; afterward verify the target and run the smallest relevant check.
- Do not use `rsync --relative` with absolute paths unless the resulting destination layout is explicitly intended and verified.
- An unreachable device remains paused. Do not attempt transfers or remote test actions at every development step.
- Reconnection follows the saved automatic/manual rule; it does not authorize repeating completed analysis.
- Standing user authorization: update file version headers for every source change, but keep changes local until the entire agreed work block is complete and verified. Then make one commit and push for that block; do not commit at intermediate steps. Updating remote devices requires an explicit user request; publication is not deployment authorization. Preserve user data and do not interrupt active transfers.

## Background Services
- Background workers perform network probes and subprocess waits outside shared state locks. HTTP reads use the last committed immutable memory snapshot; JSON persistence serializes writers without blocking readers.
- Reuse `state_transport` as the bounded native request receiver. Do not add a second orchestrator or storage writer; publish worker results through the existing state owner.
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
- build.rs includes versioned source files in src, ui, relay (including PHP), tools, scripts, tests, and the root bootstrap/build scripts. Public MP3 assets participate in the source fingerprint, while binary assets remain excluded from the version means. File Versioning exclusions still apply; documents and binary assets are excluded. Standalone local tests and diagnostic scripts ignored by Git are excluded from both the application version and source fingerprint, so installed devices match the development build.
- This calculated display does not change Cargo package versions, individual file counters, or asset cache URLs.

## Presentation Deletion
- Deleting a connection removes only that chat/device association; never alter chat visibility, appearance, inventories, or user files.
- Hiding/removing a chat presentation sets hidden and removes its associations, while retaining its label, custom icon, color, analysis and inventories. Do not use a removed flag for chat presentation.
- Permanent chat deletion is a separate, explicitly confirmed action. Use native `codex delete <UUID> --force`; purge its Oaclis links, inventories, transfer records, positions and mobile grants. Never delete project paths or apps.
- An unlinked chat resident on the master is deleted locally only: do not export its deletion intent, contact peers/mobile relay, or audit it after completion. Linked chats retain global deletion propagation.
- Keep only UUID deletion intents and peer acknowledgements in shared-settings.json for offline delivery and to prevent resurrection by stale peers or late workers. Deletion must never start analysis or content sync. Updated peers apply pending deletion after reconnecting.
- Metadata replication must not clear chat appearance when an old deletion tombstone is received.

## Device Version Alignment
- Compare source fingerprints rather than numerical application versions: the displayed means can coincide across different source builds.
- Remote app updates are explicit user actions. Preserve configuration and user data; refuse dirty or divergent checkouts, use fast-forward Git updates, then compile and restart.
- Include the committed revision in release builds by compiling after the commit before deployment.

## Unified Device Status
- Carry app availability, installed build, keep-awake state and app update progress in the existing approved SSH settings exchange. Treat this as runtime telemetry, not replicated preferences.
- Use this same snapshot in the UI, with a 30-second freshness limit. Do not create independent automatic SSH checks on hover or update-monitoring timers, or rely on LAN announcements for build comparison.
