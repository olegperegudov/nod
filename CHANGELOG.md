# Changelog

## [Unreleased]

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

**Verified by eye:** all three popover states photographed through the
`_nod_shot.mjs` harness at 2×, and the menu-bar glyph inspected at its real 22pt
size, where the first draft turned out to be an unreadable blob.
