#!/bin/sh
set -eu
cd -- "$(dirname -- "$0")"
if [ "$(id -u)" -ne 0 ]; then
    echo 'Run: sudo sh install.sh' >&2
    exit 1
fi
/usr/bin/python3 -c 'import PIL, usb.core' || {
    echo 'Install dependencies: sudo apt install python3-pil python3-usb libusb-1.0-0 fonts-dejavu-core' >&2
    exit 1
}
test -r /usr/share/fonts/truetype/dejavu/DejaVuSans.ttf
# Back up the files replaced by an upgrade, including customized service settings.
if [ -d /opt/skull-lcd ] || [ -f /etc/systemd/system/skull-lcd.service ]; then
    backup="/opt/skull-lcd-backup-$(date +%Y%m%d-%H%M%S)-$$"
    mkdir -m 0755 "$backup"
    if [ -d /opt/skull-lcd ]; then cp -a /opt/skull-lcd "$backup/"; fi
    if [ -f /etc/systemd/system/skull-lcd.service ]; then
        cp -a /etc/systemd/system/skull-lcd.service "$backup/"
    fi
    echo "Previous files saved to $backup"
fi
systemctl stop skull-lcd.service 2>/dev/null || true
install -d -m 0755 /opt/skull-lcd
install -m 0644 skull_lcd.py logo.jpg /opt/skull-lcd/
install -m 0644 skull-lcd.service /etc/systemd/system/skull-lcd.service
systemctl daemon-reload
systemctl enable --now skull-lcd.service
systemctl --no-pager --full status skull-lcd.service
