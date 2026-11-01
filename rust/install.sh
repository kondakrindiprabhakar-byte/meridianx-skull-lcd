#!/bin/sh
set -eu
cd -- "$(dirname -- "$0")"
if [ "$(id -u)" -ne 0 ]; then echo 'Run sudo sh install.sh' >&2; exit 1; fi
binary=target/release/skull-lcd
if [ ! -x "$binary" ]; then echo 'Build first: cargo build --release --locked' >&2; exit 1; fi
font=/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf
test -r "$font" || { echo 'Install fonts-dejavu-core' >&2; exit 1; }
logo=../logo.jpg
if [ ! -f "$logo" ]; then logo=logo.jpg; fi
test -r "$logo"
if [ -d /opt/skull-lcd-rust ] || [ -f /etc/systemd/system/skull-lcd-rust.service ]; then
    backup="/opt/skull-lcd-rust-backup-$(date +%Y%m%d-%H%M%S)-$$"
    mkdir -m 0755 "$backup"
    if [ -d /opt/skull-lcd-rust ]; then cp -a /opt/skull-lcd-rust "$backup/"; fi
    if [ -f /etc/systemd/system/skull-lcd-rust.service ]; then cp -a /etc/systemd/system/skull-lcd-rust.service "$backup/"; fi
    echo "Backup: $backup"
fi
# The two implementations must not own the panel simultaneously.
if systemctl cat skull-lcd.service >/dev/null 2>&1; then systemctl disable --now skull-lcd.service; fi
systemctl stop skull-lcd-rust.service 2>/dev/null || true
install -d -m 0755 /opt/skull-lcd-rust
install -m 0755 "$binary" /opt/skull-lcd-rust/skull-lcd
install -m 0644 "$logo" /opt/skull-lcd-rust/logo.jpg
install -m 0644 skull-lcd-rust.service /etc/systemd/system/
systemctl daemon-reload
systemctl enable --now skull-lcd-rust.service
systemctl --no-pager --full status skull-lcd-rust.service
