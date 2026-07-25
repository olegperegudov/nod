# Changelog

## [Unreleased]

### Fixed

**v0.1.3 aborted the moment it launched.** The popover fix converted the window
into an NSPanel without registering the plugin that hands panels out, so the
first thing the app did on startup was ask for something that was not there.
That happens inside `did_finish_launching`, where a panic cannot unwind — the
process aborted instead, which from the outside looked like an update that
uninstalled the app. The plugin now ships in the same file as the conversion,
so the two cannot be added apart.

It compiled and every test passed, because nothing in the suite ever started the
app. `tools/smoke.sh` does: it builds, launches the binary, and fails unless the
process is still alive seconds later with a finished setup in its log. It runs
in CI before anything is released, and it goes red on this exact bug — checked
by removing the fix.

**The popover now opens.** Clicking the Z's did nothing on any desktop but the
one Nod happened to launch on, and on that one the popover flashed for half a
second and vanished. Two causes, one shape: it was a plain window. A window
belongs to the Space it was born on, so on every other desktop it was opening
out of sight; and Nod is an accessory app with no Dock icon, so it never really
took focus — macOS reported the focus lost a moment later, and the handler that
closes the popover when you click away closed it instead.

It is now a non-activating NSPanel, joined to all Spaces and told that being
deactivated is not a reason to hide — the mechanism Spotlight uses, and the one
Iago already runs for its own popup. Clicking away is caught by a global mouse
monitor instead of the focus event: it only reports clicks that land in other
applications, so neither the cross inside the popover nor the menu-bar icon can
dismiss the popover out from under itself. Nod also no longer takes activation
from whatever you were working in, since nothing in the popover is typed into.

### Added

**Nod, first version.** A Mac that quietly refuses to sleep costs a battery: you
walk away expecting it to lock and doze, and five hours later it is flat. The
information was always there — `pmset -g assertions` prints it — but reading it
means opening a terminal, and it names middlemen rather than apps.

Nod puts the answer in the menu bar:

- **Three Z's, three colours.** Green: nothing is holding it. Red: something is.
  Grey: on the charger, where staying awake is the point and there is nothing to
  report.
- **Click for the list.** Every holder with what it is doing and how long it has
  been at it, and a cross that quits it (SIGTERM — the same thing ⌘Q sends).
- **A notification the moment the charger comes out**, which is the moment the
  question actually matters. Only that transition speaks; the icon has been
  saying the same thing all along.

What it deliberately does not report: assertions that expire on their own
(`caffeinate -t`), `powerd` holding sleep while the display is on (a consequence,
not a cause), and anything at all while plugged in. Judgements are made against
the battery profile from `pmset -g custom`, because the live `pmset -g` prints
`sleep 0` under a block and hides the real setting.

`coreaudiod` is resolved through its `Created for PID` line, so the list blames
the app that took the speakers rather than the audio service that holds them on
its behalf.

**Tests:** 19 Rust (holder parsing against a recorded `pmset` dump, the icon
mapping, the notification rules, file modes) and 7 vitest (the wording). Both
suites go red when the rule they cover is removed — checked by removing it.

**Hardening:** the cross passes a pid from the page, so the pid is checked
against the live holder list before anything is signalled — the popover may
close what it is showing and nothing else. The page is handed only the four
commands it calls, and the notification and updater plugins are reachable from
Rust alone.

**Verified by eye:** all three popover states photographed through the
`_nod_shot.mjs` harness at 2×, and the menu-bar glyph inspected at its real 22pt
size, where the first draft turned out to be an unreadable blob.
