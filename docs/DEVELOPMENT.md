# Development

Tauri 2: Rust around the edges, plain HTML/CSS/JS inside. No framework, no
bundler — `frontendDist` points straight at `src/`.

```bash
npm install
npm test                      # frontend logic (vitest)
cd src-tauri && cargo test --lib
npm run tauri dev
```

## What the code does

| File | Job |
| --- | --- |
| `src-tauri/src/sleep.rs` | Parses `pmset -g assertions` into holders. The filtering rules and why they exist live in the module docs. |
| `src-tauri/src/watch.rs` | Turns holders into an icon colour, and decides when a notification is warranted. Pure, so it is all under test. |
| `src-tauri/src/quit.rs` | The cross: SIGTERM to the holder's pid. |
| `src-tauri/src/lib.rs` | Tray, popover, polling, updates. |
| `src/verdict.js` | The words the popover uses. |

## Why macOS only

The whole app is a reading of `pmset -g assertions`. Windows exposes the same
idea through `powercfg /requests`, which needs an elevated prompt — a poor fit
for something that sits in the tray and answers a glance. So the build is
macOS-only on purpose, not by omission.

## Deciding what counts as a holder

Three things are filtered out, and each one is a bug someone would otherwise
report:

- **Self-expiring assertions.** `caffeinate -i -t 300` releases itself; naming
  it would be noise.
- **`powerd` holding sleep "while display is on".** That is a consequence of
  the screen being awake, not a cause.
- **Anything on the charger.** Not sleeping while plugged in is the arrangement
  working. Every judgement is made against the *battery* profile from
  `pmset -g custom`, which is also why the live `pmset -g` output is not used —
  under a block it prints `sleep 0` and hides the real setting.

`coreaudiod` is resolved through its `Created for PID` line to the app that
took the speakers. Without that the list would blame a system service.

## Starting it is a test

```bash
tools/smoke.sh              # release build, the one that ships
MODE=debug tools/smoke.sh   # faster, same check
```

Builds, launches the binary, and fails unless it is still alive six seconds
later with `setup complete` in its log. Everything that goes wrong at launch —
a plugin never registered, a panel converted before the plugin exists — happens
after `cargo test` is already green, in `did_finish_launching`, where a panic
cannot unwind and the process aborts. v0.1.3 shipped that way. CI runs this
before the release job, so it cannot happen twice.

## Icons

One shape, six menu-bar variants (three states × with and without the update
badge) plus the master used for the Dock, the DMG and the README.

```bash
npm i -D playwright        # not a dependency of the app
node tools/make_icons.mjs
npx tauri icon src/nod.png
```

The badge sits bottom-right rather than the usual top-right: the Z's climb
towards the top-right corner, so a dot up there lands on the largest stroke.

## Screenshots

`_nod_shot.mjs` (kept with the other screenshot harnesses, outside this repo) serves `src/` over http, injects
a mock `window.__TAURI__`, and photographs the popover at 2×:

```bash
SHOT=blocked OUT=/tmp/nod.png node _nod_shot.mjs   # blocked | calm | charging
```

The window height is measured from the content, exactly as the app does it —
the page asks Rust to fit the window around what it rendered, so a second
height calculation anywhere would drift out of step with the first.

## Releases

Push to `main` → CI bumps the patch version, tags, builds both architectures in
sequence, publishes the release and verifies `latest.json`. There is no separate
release ritual, and versions are never bumped by hand.

The two macOS builds run one after another because `tauri-action` merges each
platform key into the same `latest.json` asset read-modify-write; in parallel
they silently drop a key and strand half the users on an old version. The
bundles are per-arch, never universal — the manifest check fails the build if
`universal` ever appears in a URL.

## Signing

Self-signed, not notarized: first launch needs right-click → Open. The
certificate is stable across releases so macOS keeps treating updates as the
same app. CI trusts the cert as a codeSign root before building, otherwise the
bundler cannot resolve the identity at all.

Repo secrets: `TAURI_SIGNING_PRIVATE_KEY` (updater manifest), `APPLE_CERTIFICATE`,
`APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`.

## Logs

`~/Library/Application Support/nod/logs/debug.log`, mode 0600, truncated on
every launch.
