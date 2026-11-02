# Audit — release 1.0.1

Scope: Python application, USB packet generation/resource management, rendering,
systemd unit, installation/upgrade procedure, documentation, and ZIP contents.
This is a focused code review, not a security certification or hardware audit.

## Findings addressed

- Removed `After=multi-user.target` from a unit wanted by that same target to
  avoid a potential systemd ordering cycle.
- USB cleanup now attempts release, driver reattachment, and disposal even if
  the panel disconnects, without masking the original failure.
- Check expected interrupt OUT endpoint before claiming interface 1. Other USB
  interfaces are not claimed; no reset, configuration change, or flash writes.
- Removed deprecated Pillow pixel iteration from application/tests.
- Clock reserves space instead of covering the logo/message.
- Reject inconsistent blank/clock, label/content, and clock/no-refresh options.
- Unbuffered service output makes progress/errors visible in the journal.
- Added basic systemd isolation while retaining the USB access required by PyUSB.
- Added dependency checks and backups before upgrades replace team files.
- Replaced contradictory accumulated README revisions with one current procedure.
- ZIP includes an explicit file list; no caches, virtual environments, scratch
  downloads, or machine-specific logs. Includes SHA-256 file manifest.

## Verification

Seven hardware-free regression tests cover RGB565 primary colors; full-frame
phase/sequence/offset/length and image reconstruction; complete non-overlapping
partial updates; malformed packets; branded/clock rendering; and cleanup after
USB disconnect. Also checked CLI error handling, PNG preview, shell syntax,
systemd unit verification, ZIP integrity, manifest checks, and test execution
from an extracted release. See README for repeatable test commands.

## Remaining limits

- Physical USB transfers and this release's service sandbox were not exercised
  here. User confirmed an earlier logo/service version works on their PC.
- Periodic refresh is a workaround for the observed demo fallback. Root cause
  and a permanent controller demo-disable command are not verified.
- Full-frame offset wraps at 64 KiB per the documented firmware protocol; test
  checks encoding/reconstruction, not firmware interpretation on every revision.
- Only one attached matching display is supported; PyUSB selects its first match.
- Service requires root; a restricted device-user setup has not been validated.
- USB timeout/disconnect makes the process exit; systemd retries rather than an
  in-process reconnect. Missing images likewise retry until corrected.
- Installer is not an atomic transaction. Backups support manual rollback if
  installation fails. It replaces local service customizations with defaults.
- No permanent logo change, firmware date correction, shutdown control, RGB
  control, portrait layout, or monitoring dashboard is implemented.
- Dependency minimums are declared, not locked; automated checks used Python
  3.13.5, Pillow 13.0.0, PyUSB 1.3.1. Target distro packages must be validated.

## 1.0.1 stop handling

Added black full-frame upload on SIGTERM/SIGINT, bounded by a total deadline and
per-transfer timeout, before USB release. Cleanup remains guaranteed if the
black upload fails. Service explicitly uses SIGTERM and a 30-second stop timeout.
Added a black-frame/timeout regression test. This does not implement permanent
blackout of standby-powered firmware or force-kill handling.

## Rust edition 0.1.0

The native port lives under `rust/`. Eight tests cover byte-level compatibility
with the Python full/partial packets, reconstruction, black frames, CLI/color
validation, stop interruption, and text/clock rendering. Uses safe Rust APIs,
libusb via rusb interrupt writes, auto-detach scoped to interface 1, and a finite
cleanup deadline. Zero-length USB timeouts are avoided. No network listener.
The dependency lockfile is committed; libusb is built from vendored crate source.
Rust runtime is not dependent on Python. Installer disables Python's service
before enabling Rust's; rollback steps are in the Rust README. No changes are
made to firmware or stored-logo memory. Rust USB operation, new installer, and
signal/shutdown behavior still require physical acceptance testing. Packet parity
does not prove every controller revision handles the protocol identically.

## Brightness update — Python 1.0.2 / Rust 0.1.1

Added integer software brightness 0–100 with default 60 as requested. Both
renderers use the same integer scaling/rounding after all compositing, so
re-rendering the clock does not compound dimming. Added endpoint/rounding tests
and Rust CLI range tests. No guessed USB commands or backlight control added.
Python has eight tests; Rust has nine tests. Physical dimming needs host testing.
