"""Hardware-free regression tests. Run with python3 -m unittest -v."""
import argparse
import contextlib
import io
import struct
import unittest
from pathlib import Path
from PIL import Image
import skull_lcd as lcd


class ProtocolTests(unittest.TestCase):
    def setUp(self):
        self.image = Image.new('RGB', lcd.SIZE)
        self.image.putdata([(x % 256, y % 256, (x + y) % 256)
                            for y in range(170) for x in range(320)])

    def test_rgb565_primary_colors(self):
        im = Image.new('RGB', (4, 1))
        im.putdata([(255, 0, 0), (0, 255, 0), (0, 0, 255), (255, 255, 255)])
        self.assertEqual(lcd.rgb565(im), bytes.fromhex('f80007e0001fffff'))

    def test_redraw_reconstructs_full_image(self):
        packets = list(lcd.redraw_packets(self.image))
        self.assertEqual(len(packets), 27)
        recovered = b''
        for i, p in enumerate(packets):
            self.assertEqual(len(p), 4104)
            sig, cmd, phase, seq, offset, n = struct.unpack('<BBBBHH', p[:8])
            self.assertEqual((sig, cmd, seq, offset), (0x55, 0xA3, i+1, (i*4096)&65535))
            self.assertEqual(phase, 0xF0 if i == 0 else 0xF2 if i == 26 else 0xF1)
            self.assertEqual(n, 2304 if i == 26 else 4096)
            recovered += p[8:8+n]
        self.assertEqual(recovered, lcd.rgb565(self.image))

    def test_partial_updates_cover_each_pixel_once(self):
        pixels = {}
        for p in lcd.frame_packets(self.image):
            self.assertEqual(len(p), 4104)
            sig, cmd, x, y, w, h = struct.unpack('<BBHHBB', p[:8])
            self.assertEqual((sig, cmd), (0x55, 0xA2))
            for row in range(h):
                for col in range(w):
                    xy = x+col, y+row
                    self.assertNotIn(xy, pixels)
                    off = 8+(row*w+col)*2
                    pixels[xy] = p[off:off+2]
        self.assertEqual(len(pixels), 320*170)
        self.assertEqual(b''.join(pixels[x,y] for y in range(170) for x in range(320)),
                         lcd.rgb565(self.image))

    def test_invalid_packets_rejected(self):
        for header, data in [(bytes(7), b''), (bytes(8), bytes(4097))]:
            with self.assertRaises(ValueError):
                lcd.packet(header, data)

    def test_branded_render_and_clock(self):
        args = argparse.Namespace(image=Path(__file__).with_name('logo.jpg'),
                                  text=None, blank=False, background='white',
                                  font=None, color='black', label='MeridianX',
                                  label_color='#092681', clock=False, size=40)
        im = lcd.render(args)
        self.assertEqual(im.size, lcd.SIZE)
        self.assertLess(min(im.crop((174, 60, 315, 110)).tobytes()), 100)
        args.clock = True
        self.assertEqual(lcd.render(args).size, lcd.SIZE)

    def test_stop_sends_black_frame_with_bounded_writes(self):
        display = lcd.Display.__new__(lcd.Display)
        sent = []
        display.send = lambda data, timeout: sent.append((data, timeout))
        with contextlib.redirect_stdout(io.StringIO()):
            display.blank_on_stop()
        self.assertEqual(len(sent), 27)
        for data, timeout in sent:
            self.assertTrue(0 < timeout <= 1000)
            self.assertEqual(data[8:], bytes(4096))
        self.assertEqual(sent[0][0][2], 0xF0)
        self.assertEqual(sent[-1][0][2], 0xF2)

    def test_cleanup_continues_after_disconnect(self):
        calls = []
        class Util:
            def release_interface(self, *_):
                calls.append('release')
                raise OSError('disconnected')
            def dispose_resources(self, *_):
                calls.append('dispose')
        class Device:
            def attach_kernel_driver(self, *_):
                calls.append('attach')
        display = lcd.Display.__new__(lcd.Display)
        display.util, display.dev = Util(), Device()
        display.claimed = display.detached = True
        with contextlib.redirect_stderr(io.StringIO()):
            display.close()
        self.assertEqual(calls, ['release', 'attach', 'dispose'])
        self.assertFalse(display.claimed)
        self.assertFalse(display.detached)

if __name__ == '__main__':
    unittest.main()
