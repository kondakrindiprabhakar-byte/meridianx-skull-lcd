# MeridianX / Roblogix LCD — version 1.0.1

Display the supplied Roblogix logo with “MeridianX” beside it on a Skullsaints
HT32 front-panel LCD. The label is drawn by the application; the supplied
`logo.jpg` remains unchanged. The included `logo-preview.png` shows the exact
320×170 framebuffer layout. This utility does not control the RGB light strip.

![MeridianX LCD preview](logo-preview.png)

## Rust edition

A native Rust implementation is available in [rust/](rust/README.md), with its own
installer and service. It supports the same USB protocol and stop-to-black
behavior; keep only one implementation running.

## Supported setup

- Linux with systemd; installation commands target Debian/Ubuntu.
- USB LCD `04d9:fd01`, interface 1, interrupt OUT endpoint `0x02`, 320×170 RGB565.
- Python 3.9+, Pillow, PyUSB, libusb, and DejaVu Sans font.
- Run on the physical host with USB access, with only one panel program running.

The user confirmed the preceding logo/service version works on their computer.
This release's branding and audit fixes were tested without physical USB access;
complete the hardware acceptance checks below on each target model before rollout.

## Install or upgrade

Extract `skull-lcd.zip`, open a terminal inside the extracted `skull-lcd` folder,
and stop any manually running copy with Ctrl+C.

```bash
sudo apt update
sudo apt install python3-pil python3-usb libusb-1.0-0 fonts-dejavu-core
sudo sh install.sh
```

The installer copies the program and logo to `/opt/skull-lcd`, installs the
service in `/etc/systemd/system/skull-lcd.service`, and enables and starts it.
On upgrade it backs up existing application files and service settings under
`/opt/skull-lcd-backup-TIMESTAMP-PID` before replacement. It installs the supplied
service settings, so reapply any intentional local customization afterward.
Dependencies are checked before replacement; the installer does not install
packages itself. Review the terminal status after installation.

```bash
systemctl is-enabled skull-lcd
systemctl status skull-lcd --no-pager
journalctl -u skull-lcd -b -n 50 --no-pager
```

No firmware flashing occurs. The service runs as root to claim the USB interface;
it has a read-only system filesystem, hidden home directories, private temporary
storage, and no permission to gain additional privileges. It retries failures
every five seconds. Logs go to the system journal.

## Preview or run manually

Commands below assume the extracted folder is the current directory.
A preview requires no root or USB access:

```bash
/usr/bin/python3 skull_lcd.py --image logo.jpg --label MeridianX --background white --preview preview.png
```

For a physical screen test, first stop the service:

```bash
sudo systemctl stop skull-lcd
sudo /usr/bin/python3 skull_lcd.py --image logo.jpg --label MeridianX --background white
```

Leave the command running. Ctrl+C stops it. Restart the service afterward with
`sudo systemctl start skull-lcd`.

Other examples:

```bash
sudo /usr/bin/python3 skull_lcd.py --text 'MeridianX' --color cyan
sudo /usr/bin/python3 skull_lcd.py --image logo.jpg --label MeridianX --background white --clock --color black
sudo /usr/bin/python3 skull_lcd.py --blank
```

The clock uses the Linux host's local timezone and date; it updates during each
periodic refresh. It reserves a bottom strip so the logo is not covered. With
white backgrounds use `--color black` for the clock. `--blank` paints black;
it does not turn off the backlight. Text fits automatically; image aspect ratio
is preserved. Use `--font /absolute/path/font.ttf` for another installed font.
`--label` requires `--image`. See `--help` for all options.

## Customize startup

Edit `/etc/systemd/system/skull-lcd.service` with `sudo systemctl edit --full skull-lcd`.
The default command is:

```text
ExecStart=/usr/bin/python3 -u /opt/skull-lcd/skull_lcd.py --image /opt/skull-lcd/logo.jpg --background white --label MeridianX
```

Keep assets under `/opt/skull-lcd`; the service cannot read home directories.
Use absolute paths. Then apply your changes:

```bash
sudo systemctl daemon-reload
sudo systemctl restart skull-lcd
```

## Troubleshooting

- **Factory dashboard / wrong 2019 date:** this is not rendered by our program.
  Firmware fallback or another writer may be responsible. The application sends
  a full frame on startup, a heartbeat every second, and partial updates covering
  the complete screen every two seconds after the preceding upload finishes.
  This is a recovery workaround, not a verified permanent demo-disable command.
  If alternating screens persist, collect journal output and check for other
  panel software. `--refresh 0` disables periodic restoration for diagnosis;
  `--partial` uses partial updates for the initial upload as well.
- **Logo after shutdown:** the controller may have USB standby power and restore
  its own firmware logo. The Linux service cannot control it when Linux is off.
  No verified stored-logo replacement command is implemented.
- **USB access / busy:** stop other copies and panel services; run on the physical
  host as root. Match the USB VID/PID; do not send LCD data to the CH340 or audio
  devices. Run `lsusb` and inspect `journalctl -u skull-lcd`.
- **Repeated service restarts:** inspect the journal for missing image/font,
  USB write failures, or permissions. The service retries; a retry is not proof
  that the underlying error is fixed.
- **Missing dependency:** install the packages above with apt. `requirements.txt`
  is provided for development environments, but the service uses `/usr/bin/python3`.
- **Text too small:** the complete circular logo occupies about 168×170 pixels.
  Fine lettering has limited readability at this resolution.

`--once` exits after upload and cannot maintain the content. The USB write timeout
is ten seconds per transfer; the service allows 30 seconds for stopping. No root
password, network service, remote control, or telemetry is included.

## Team acceptance procedure

1. Verify ZIP checksum against the supplied `.sha256` file. Extract, inspect
   `AUDIT.md`, and install dependencies.
2. Run `/usr/bin/python3 -m unittest -v` in the extracted folder (no USB needed).
3. Preview the branded image and confirm the logo and MeridianX text.
4. Stop other panel software; install with `sudo sh install.sh`.
5. Watch the physical screen for at least two minutes. Confirm the logo remains,
   no demo screen interrupts it, and the journal has no USB failures.
6. Reboot and confirm the service is active and the branded screen returns.
7. Test stop/start and, if relevant, suspend/resume on the target PC. Record the
   PC model, OS, USB ID, release version, and result before wider rollout.

These physical checks have not been performed by the packaging environment.

## Rollback / remove

To roll back an upgrade, stop the service and restore the backed-up program,
logo, and service file from the printed backup directory to their original paths.
Then run `sudo systemctl daemon-reload` and `sudo systemctl restart skull-lcd`.

To uninstall:

```bash
sudo systemctl disable --now skull-lcd
sudo rm /etc/systemd/system/skull-lcd.service
sudo systemctl daemon-reload
sudo rm -r /opt/skull-lcd
```

Backups are retained for manual review/removal. Stopping/removing the utility
allows the panel firmware to choose its display; factory-logo restoration timing
is firmware dependent.

## Protocol references and ownership

- https://github.com/ananthb/ht32-panel — Skullsaints support and interface identification.
- https://github.com/tjaworski/AceMagic-S1-LED-TFT-Linux — reverse-engineered protocol.

The application uses documented command formats; it does not bundle either
upstream project or its source. Upstream projects have their own licenses.
Pillow and PyUSB are installed separately with their own licenses. The supplied
Roblogix logo is user-provided; this package grants no additional trademark or
artwork rights. Confirm redistribution rights before sharing outside your team.

## Black screen on service stop / shutdown (1.0.1)

Normal service stop, restart, Ctrl+C, and Linux shutdown send SIGTERM/SIGINT.
The application then uploads a complete black frame before releasing USB.
Run `sudo systemctl stop skull-lcd` to check this behavior; inspect the journal
for `LCD cleared to black before stop.` A restart clears briefly before drawing
the logo again. Install the updated program AND service with `sudo sh install.sh`.

The black upload has a 12-second total budget and at most one second per write;
the service allows 30 seconds to finish an in-flight write and cleanup. Failures
are logged and USB resources are still released. `--once` retains its uploaded
image when it exits normally; `--no-blank-on-stop` opts out of signal-stop clearing.

This sends black pixels, not a backlight-off or persistent firmware command.
The panel can later restore its own warning/logo while powered by standby USB.
Therefore remaining black indefinitely after shutdown cannot be guaranteed with
the currently verified protocol. SIGKILL (`kill -9`), a USB disconnect, or sudden
power loss cannot run cleanup. Physical shutdown/stop behavior needs host testing.
