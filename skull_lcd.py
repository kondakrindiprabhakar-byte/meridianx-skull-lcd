#!/usr/bin/env python3
"""Custom content for the 04d9:fd01 HT32 front panel (320x170)."""
import argparse
from datetime import datetime
import signal
import struct
import sys
import threading
import time
from pathlib import Path

VERSION = "1.0.1"
SIZE = (320, 170)

def packet(header, payload=b''):
    if len(header) != 8 or len(payload) > 4096:
        raise ValueError('Invalid LCD packet')
    return header + payload + bytes(4096 - len(payload))

def rgb565(image):
    result = bytearray()
    data = image.convert('RGB').tobytes()
    for index in range(0, len(data), 3):
        r, g, b = data[index:index+3]
        result.extend(struct.pack('>H', ((r >> 3) << 11) | ((g >> 2) << 5) | (b >> 3)))
    return bytes(result)

def redraw_packets(image):
    data = rgb565(image)
    for index, offset in enumerate(range(0, len(data), 4096)):
        chunk = data[offset:offset + 4096]
        phase = 0xF0 if index == 0 else (0xF2 if offset + len(chunk) == len(data) else 0xF1)
        yield packet(struct.pack('<BBBBHH', 0x55, 0xA3, phase, index + 1,
                                 offset & 0xFFFF, len(chunk)), chunk)


def frame_packets(image):
    # Small rectangular writes avoid the firmware's wrapping full-frame offsets.
    for y in range(0, 170, 12):
        h = min(12, 170 - y)
        for x in (0, 160):
            yield packet(struct.pack('<BBHHBB', 0x55, 0xA2, x, y, 160, h),
                         rgb565(image.crop((x, y, x + 160, y + h))))

def render(args):
    from PIL import Image, ImageDraw, ImageFont, ImageOps
    image = Image.new('RGB', SIZE, args.background)
    content_height = 145 if args.clock else 170
    if args.image:
        with Image.open(args.image) as source:
            source = ImageOps.exif_transpose(source).convert('RGBA')
            source = ImageOps.contain(source, (168 if args.label else 320, content_height))
            x = (168-source.width)//2 if args.label else (320-source.width)//2
            image.paste(source, (x, (content_height-source.height)//2), source)
        if args.label:
            draw = ImageDraw.Draw(image)
            for size in range(28, 7, -1):
                font = ImageFont.truetype(args.font or '/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf', size)
                box = draw.textbbox((0, 0), args.label, font=font)
                if box[2]-box[0] <= 142:
                    break
            else:
                raise ValueError('Label is too long for the display')
            draw.text((244, content_height//2), args.label, anchor='mm', font=font, fill=args.label_color)
    if args.text:
        draw = ImageDraw.Draw(image)
        font_path = args.font or '/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf'
        for size in range(args.size, 7, -1):
            font = ImageFont.truetype(font_path, size)
            box = draw.multiline_textbbox((0, 0), args.text, font=font, spacing=5, align='center')
            if box[2]-box[0] <= 300 and box[3]-box[1] <= content_height-20:
                break
        else:
            raise ValueError('Message is too long. Add line breaks or shorten it.')
        draw.multiline_text(((320-box[2]+box[0])/2-box[0],
                             (content_height-box[3]+box[1])/2-box[1]),
                            args.text, font=font, fill=args.color, spacing=5, align='center')
    if args.clock:
        draw = ImageDraw.Draw(image)
        font = ImageFont.truetype(args.font or '/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf', 16)
        draw.rectangle((0, 145, 319, 169), fill=args.background)
        draw.text((160, 157), datetime.now().strftime('%Y-%m-%d  %H:%M:%S'), anchor='mm', font=font, fill=args.color)
    return image

class Display:
    def __init__(self):
        import usb.core
        import usb.util
        self.util = usb.util
        self.dev = usb.core.find(idVendor=0x04D9, idProduct=0xFD01)
        if self.dev is None:
            raise RuntimeError('LCD 04d9:fd01 not found. Run on the host with USB access.')
        self.claimed = False
        self.detached = False
        try:
            # Interface 1, OUT endpoint 0x02 is the LCD, not the consumer HID.
            interface = self.dev.get_active_configuration()[(1, 0)]
            if not any(ep.bEndpointAddress == 0x02 and (ep.bmAttributes & 3) == 3 for ep in interface):
                raise RuntimeError('Expected interrupt OUT endpoint 0x02 is missing')
            if self.dev.is_kernel_driver_active(1):
                self.dev.detach_kernel_driver(1)
                self.detached = True
            usb.util.claim_interface(self.dev, 1)
            self.claimed = True
        except Exception:
            self.close()
            raise

    def send(self, data, timeout=10000):
        # USB transport carries 4104 bytes; the HID API's leading report ID is omitted.
        written = self.dev.write(0x02, data, timeout=timeout)
        if written != len(data):
            raise RuntimeError(f'Short USB write: {written}/{len(data)}')

    def heartbeat(self):
        now = datetime.now()
        self.send(packet(bytes((0x55, 0xA1, 0xF2, now.hour, now.minute, now.second, 0, 0))))

    def blank_on_stop(self):
        from PIL import Image
        # Bound the whole cleanup, rather than allowing 27 ten-second writes.
        deadline = time.monotonic() + 12
        for data in redraw_packets(Image.new('RGB', SIZE, 'black')):
            remaining = int((deadline - time.monotonic()) * 1000)
            if remaining <= 0:
                raise TimeoutError('Timed out clearing LCD during stop')
            self.send(data, timeout=min(1000, remaining))
        print('LCD cleared to black before stop.', flush=True)

    def close(self):
        # Disconnects during shutdown must not mask the original USB error.
        for action in (
            lambda: self.util.release_interface(self.dev, 1) if self.claimed else None,
            lambda: self.dev.attach_kernel_driver(1) if self.detached else None,
            lambda: self.util.dispose_resources(self.dev),
        ):
            try:
                action()
            except Exception as exc:
                print(f'USB cleanup warning: {exc}', file=sys.stderr)
        self.claimed = False
        self.detached = False

def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--version', action='version', version=VERSION)
    content = p.add_mutually_exclusive_group(required=True)
    content.add_argument('--text', help='Message; quote it and use actual newlines for multiple lines')
    content.add_argument('--image', type=Path, help='PNG/JPEG/logo; fitted without stretching')
    content.add_argument('--blank', action='store_true', help='Replace logo with black (backlight stays on)')
    p.add_argument('--label', help='Brand name beside the logo; requires --image')
    p.add_argument('--label-color', default='#092681')
    p.add_argument('--background', default='black')
    p.add_argument('--color', default='white')
    p.add_argument('--font', help='Path to a TrueType font')
    p.add_argument('--size', type=int, default=40, choices=range(8, 151), metavar='8..150')
    p.add_argument('--preview', type=Path, help='Save PNG and exit without touching USB')
    p.add_argument('--refresh', type=float, default=2, metavar='SECONDS', help='Reapply image periodically (default: 2; 0 disables)')
    p.add_argument('--clock', action='store_true', help='Draw current Linux local date/time along the bottom')
    p.add_argument('--partial', action='store_true', help='Use the original partial-update upload for comparison')
    p.add_argument('--heartbeat', type=float, default=1.0, metavar='SECONDS', help='Heartbeat interval, 0.1 to 5 seconds (default: 1)')
    p.add_argument('--no-blank-on-stop', action='store_true', help='Keep framebuffer on SIGTERM/Ctrl+C instead of clearing it')
    p.add_argument('--once', action='store_true', help='Exit after upload; firmware may later restore its default display')
    args = p.parse_args()
    if args.label and not args.image:
        p.error('--label requires --image')
    if args.blank and args.clock:
        p.error('--blank cannot be combined with --clock')
    if args.clock and not args.refresh and not args.once and not args.preview:
        p.error('--clock requires --refresh greater than zero')
    if not 0 <= args.refresh <= 3600:
        p.error('--refresh must be between 0 and 3600 seconds')
    if not 0.1 <= args.heartbeat <= 5:
        p.error('--heartbeat must be between 0.1 and 5 seconds')
    if args.blank:
        args.background = 'black'
    display = None
    stop = threading.Event()
    try:
        image = render(args)
        if args.preview:
            image.save(args.preview)
            print(f'Saved {args.preview}')
            return 0
        display = Display()
        for sig in (signal.SIGINT, signal.SIGTERM):
            signal.signal(sig, lambda *_: stop.set())
        if stop.wait(1):
            return 0
        display.send(packet(bytes((0x55, 0xA1, 0xF1, 1, 0, 0, 0, 0))))
        display.heartbeat()
        upload = frame_packets(image) if args.partial else redraw_packets(image)
        for data in upload:
            if stop.is_set():
                return 0
            display.send(data)
        display.heartbeat()
        print('Custom display uploaded. Keep this running to maintain it; Ctrl+C stops.')
        if not args.once:
            next_refresh = time.monotonic() + args.refresh
            refresh_count = 0
            while not stop.wait(args.heartbeat):
                display.heartbeat()
                if args.refresh and time.monotonic() >= next_refresh:
                    # Do not interrupt an A3 transaction with config commands.
                    # Periodic A2 updates restore the framebuffer after firmware fallback.
                    if args.clock:
                        image = render(args)
                    for data in frame_packets(image):
                        if stop.is_set():
                            return 0
                        display.send(data)
                    display.heartbeat()
                    refresh_count += 1
                    if refresh_count == 1 or refresh_count % 30 == 0:
                        print(f'{datetime.now().isoformat(timespec="seconds")} content refresh {refresh_count}', flush=True)
                    next_refresh = time.monotonic() + args.refresh
        return 0
    except Exception as exc:
        print(f'Error: {exc}', file=sys.stderr)
        print('Install python3-pil, python3-usb and libusb-1.0-0. For USB permission errors, try sudo. Stop other panel software first.', file=sys.stderr)
        return 1
    finally:
        if display is not None:
            try:
                if stop.is_set() and not args.no_blank_on_stop:
                    try:
                        display.blank_on_stop()
                    except Exception as exc:
                        print(f'Could not clear LCD on stop: {exc}', file=sys.stderr)
            finally:
                display.close()

if __name__ == '__main__':
    sys.exit(main())
