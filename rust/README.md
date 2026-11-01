# Skullsaints LCD — Rust edition 0.1.0

Native Linux port of the Python application in this repository. Supports the
HT32 USB LCD `04d9:fd01`, interface 1 / interrupt OUT `0x02`, 320×170 RGB565.
This is the LCD, not the CH340 RGB light controller.

Features: PNG/JPEG logos (EXIF orientation and alpha handled), MeridianX label,
multiline text with automatic fitting, local date/time, previews without USB,
full initial upload, partial periodic restoration, heartbeat, and bounded black
frame cleanup on SIGTERM/SIGINT. USB backend uses [rusb interrupt transfers](https://docs.rs/rusb/latest/rusb/struct.DeviceHandle.html).
The supplied logo is unchanged; text is composed
beside it. Font rasterization differs slightly from Python/Pillow.

## Build from repository

Use a current stable Rust toolchain. The lockfile is included. On Debian/Ubuntu:

```bash
sudo apt install build-essential pkg-config fonts-dejavu-core
cd rust
cargo build --release --locked
cargo test --locked
```

The first build downloads dependencies. libusb is compiled from its bundled
source by `rusb`; Python/Pillow/PyUSB are not runtime dependencies of this binary.
Build on the target architecture, or provide a matching binary. This version has
been compiled on x86_64 Linux; other targets have not been validated.

## Preview / manual use

From the repository's `rust` directory:

```bash
./target/release/skull-lcd --image ../logo.jpg --label MeridianX --background white --preview preview.png
sudo systemctl stop skull-lcd skull-lcd-rust
sudo ./target/release/skull-lcd --image ../logo.jpg --label MeridianX --background white
```

Ignore a stop error for a service that is not installed. Leave the foreground
command running; Ctrl+C attempts to clear the screen to black.

```bash
sudo ./target/release/skull-lcd --text 'MeridianX' --color cyan
sudo ./target/release/skull-lcd --image ../logo.jpg --label MeridianX --background white --clock --color black
sudo ./target/release/skull-lcd --blank
./target/release/skull-lcd --help
```

Basic named colors and `#RRGGBB` are accepted. `--font` takes a TrueType font path.
Only PNG/JPEG input is enabled. Default heartbeat is one second; refresh is two
seconds after completion of the previous upload. `--refresh 0` disables periodic
restoration; `--partial` uses partial updates for initial drawing. `--once` exits
normally after upload and leaves that framebuffer; `--no-blank-on-stop` disables
black cleanup on signals. Clock uses Linux local time, not the firmware demo date.

## Switch the startup service to Rust

Stop any manually running copy, then from this directory after building:

```bash
sudo sh install.sh
systemctl status skull-lcd-rust --no-pager
journalctl -u skull-lcd-rust -b -n 50 --no-pager
```

The installer backs up an existing Rust installation, disables/stops the Python
`skull-lcd` service, installs the Rust binary/logo under `/opt/skull-lcd-rust`,
and enables `skull-lcd-rust`. The original Python program/service files remain
available. Run only one implementation. An installation failure after disabling
Python can be recovered with `sudo systemctl enable --now skull-lcd`.

The unit runs as root for USB access with the same filesystem restrictions as
the Python service. It retries failures every five seconds. Normal shutdown
sends SIGTERM and allows 30 seconds for an in-flight USB write plus cleanup.
Black cleanup has a 12-second total budget, one-second transfer limits, and
always releases the interface afterward. USB timeouts are clamped above zero
because libusb interprets zero as an infinite wait.

## Team ZIP

The Rust ZIP contains the project source, logo, and a prebuilt binary at
`rust/target/release/skull-lcd`; enter `rust/` and run `sudo sh install.sh`
to install without building. The included
x86_64 GNU/Linux binary was built on Debian 13. It is not a universal/static
Linux binary: older glibc systems or another CPU architecture may need a local
build. Use `ldd target/release/skull-lcd` to inspect runtime dependencies.

## Validate on hardware

Software tests compare full and partial packet bytes against Python v1.0.1
golden checksums and reconstruct the framebuffer, check black frames, CLI/color
validation, interruption, and text/clock rendering. USB transfers and this new
Rust service have not been tested on the physical panel in this environment.
The user confirmed the earlier Python stop-to-black implementation works.

Before rollout: preview the logo; run manually for two minutes; inspect logs;
stop and confirm black; install the service; reboot; confirm startup and normal
shutdown. The previously observed firmware disconnection warning is not proven
fixed by changing language. Periodic redraw remains a workaround. SIGKILL,
sudden power loss, and USB removal cannot run cleanup. Standby-powered firmware
may later restore its warning/logo; no permanent firmware change is implemented.
Portrait UI/rotation controls and RGB lights are outside this port's scope.

## Roll back to Python / uninstall Rust

```bash
sudo systemctl disable --now skull-lcd-rust
sudo systemctl enable --now skull-lcd
```

To remove Rust files after stopping it:

```bash
sudo rm /etc/systemd/system/skull-lcd-rust.service
sudo systemctl daemon-reload
sudo rm -r /opt/skull-lcd-rust
```

Backups remain for manual review. See the parent README for artwork ownership,
protocol sources, and Python installation. Rust dependencies keep their own
licenses; this port does not bundle the upstream HT32 panel project's source.
