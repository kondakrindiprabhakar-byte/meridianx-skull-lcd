use ab_glyph::FontArc;
use anyhow::{bail, Context, Result};
use chrono::{Local, Timelike};
use clap::{ArgGroup, Parser};
use image::{imageops, ImageDecoder, Rgb, RgbImage};
use imageproc::drawing::{draw_text_mut, text_size};
use rusb::{DeviceHandle, GlobalContext, TransferType};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

const WIDTH: u32 = 320;
const HEIGHT: u32 = 170;
const PACKET_SIZE: usize = 4104;
type Packet = [u8; PACKET_SIZE];

#[derive(Parser, Debug)]
#[command(
    version,
    about = "Custom text/logo for Skullsaints HT32 USB LCD (04d9:fd01)"
)]
#[command(group(ArgGroup::new("content").required(true).args(["text", "image", "blank"])))]
struct Args {
    #[arg(long)]
    text: Option<String>,
    #[arg(long)]
    image: Option<PathBuf>,
    #[arg(long)]
    blank: bool,
    #[arg(long, requires = "image", conflicts_with_all = ["text", "blank"])]
    label: Option<String>,
    #[arg(long, default_value = "#092681", value_parser = color)]
    label_color: Rgb<u8>,
    #[arg(long, default_value = "black", value_parser = color)]
    background: Rgb<u8>,
    #[arg(long, default_value = "white", value_parser = color)]
    color: Rgb<u8>,
    #[arg(
        long,
        default_value = "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"
    )]
    font: PathBuf,
    #[arg(long, default_value_t = 40, value_parser = clap::value_parser!(u32).range(8..=150))]
    size: u32,
    #[arg(long)]
    preview: Option<PathBuf>,
    #[arg(long, default_value_t = 2.0)]
    refresh: f64,
    #[arg(long, default_value_t = 1.0)]
    heartbeat: f64,
    #[arg(long, conflicts_with = "blank")]
    clock: bool,
    #[arg(long)]
    partial: bool,
    #[arg(long)]
    once: bool,
    #[arg(long)]
    no_blank_on_stop: bool,
}

fn color(s: &str) -> std::result::Result<Rgb<u8>, String> {
    let hex = match s.to_ascii_lowercase().as_str() {
        "black" => "000000",
        "white" => "ffffff",
        "red" => "ff0000",
        "green" => "008000",
        "blue" => "0000ff",
        "cyan" => "00ffff",
        "orange" => "ffa500",
        "yellow" => "ffff00",
        _ => s.strip_prefix('#').unwrap_or(s),
    };
    if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("use a named basic color or #RRGGBB".into());
    }
    let n = u32::from_str_radix(hex, 16).map_err(|e| e.to_string())?;
    Ok(Rgb([(n >> 16) as u8, (n >> 8) as u8, n as u8]))
}

fn packet(header: [u8; 8], payload: &[u8]) -> Packet {
    assert!(payload.len() <= 4096);
    let mut p = [0; PACKET_SIZE];
    p[..8].copy_from_slice(&header);
    p[8..8 + payload.len()].copy_from_slice(payload);
    p
}

fn rgb565(im: &RgbImage) -> Vec<u8> {
    im.pixels()
        .flat_map(|p| {
            (((p[0] as u16 >> 3) << 11) | ((p[1] as u16 >> 2) << 5) | (p[2] as u16 >> 3))
                .to_be_bytes()
        })
        .collect()
}

fn redraw(im: &RgbImage) -> Vec<Packet> {
    assert_eq!(im.dimensions(), (WIDTH, HEIGHT));
    let bytes = rgb565(im);
    let chunks = bytes.chunks(4096);
    let count = chunks.len();
    chunks
        .enumerate()
        .map(|(i, data)| {
            let off = ((i * 4096) as u16).to_le_bytes();
            let len = (data.len() as u16).to_le_bytes();
            packet(
                [
                    0x55,
                    0xa3,
                    if i == 0 {
                        0xf0
                    } else if i + 1 == count {
                        0xf2
                    } else {
                        0xf1
                    },
                    (i + 1) as u8,
                    off[0],
                    off[1],
                    len[0],
                    len[1],
                ],
                data,
            )
        })
        .collect()
}

fn partial(im: &RgbImage) -> Vec<Packet> {
    assert_eq!(im.dimensions(), (WIDTH, HEIGHT));
    let mut out = Vec::new();
    for y in (0..HEIGHT).step_by(12) {
        let h = 12.min(HEIGHT - y);
        for x in [0u32, 160] {
            let xb = (x as u16).to_le_bytes();
            let yb = (y as u16).to_le_bytes();
            let region = imageops::crop_imm(im, x, y, 160, h).to_image();
            out.push(packet(
                [0x55, 0xa2, xb[0], xb[1], yb[0], yb[1], 160, h as u8],
                &rgb565(&region),
            ));
        }
    }
    out
}

fn centered(
    im: &mut RgbImage,
    font: &FontArc,
    text: &str,
    size: u32,
    cx: i32,
    cy: i32,
    fill: Rgb<u8>,
) {
    let (w, h) = text_size(size as f32, font, text);
    draw_text_mut(
        im,
        fill,
        cx - w as i32 / 2,
        cy - h as i32 / 2,
        size as f32,
        font,
        text,
    );
}

fn render(a: &Args) -> Result<RgbImage> {
    let bg = if a.blank {
        Rgb([0, 0, 0])
    } else {
        a.background
    };
    let mut im = RgbImage::from_pixel(WIDTH, HEIGHT, bg);
    let ch = if a.clock { 145 } else { 170 };
    let needs_font = a.text.is_some() || a.label.is_some() || a.clock;
    let font = if needs_font {
        Some(
            FontArc::try_from_vec(std::fs::read(&a.font).context("Read TrueType font")?)
                .map_err(|_| anyhow::anyhow!("Invalid font"))?,
        )
    } else {
        None
    };
    if let Some(path) = &a.image {
        let mut decoder = image::ImageReader::open(path)?
            .with_guessed_format()?
            .into_decoder()?;
        let orientation = decoder.orientation()?;
        let mut source = image::DynamicImage::from_decoder(decoder)?;
        source.apply_orientation(orientation);
        let fitted = source
            .resize(
                if a.label.is_some() { 168 } else { 320 },
                ch,
                imageops::FilterType::Lanczos3,
            )
            .to_rgba8();
        let x = (if a.label.is_some() { 168 } else { 320 } - fitted.width()) / 2;
        let y = (ch - fitted.height()) / 2;
        for (px, py, p) in fitted.enumerate_pixels() {
            let alpha = p[3] as u32;
            let blended = Rgb(std::array::from_fn(|i| {
                ((p[i] as u32 * alpha + bg[i] as u32 * (255 - alpha) + 127) / 255) as u8
            }));
            im.put_pixel(x + px, y + py, blended);
        }
    }
    if let Some(label) = &a.label {
        let font = font.as_ref().unwrap();
        let size = (8..=28)
            .rev()
            .find(|s| text_size(*s as f32, font, label).0 <= 142)
            .context("Label too long")?;
        centered(
            &mut im,
            font,
            label,
            size,
            244,
            ch as i32 / 2,
            a.label_color,
        );
    }
    if let Some(text) = &a.text {
        let font = font.as_ref().unwrap();
        let lines: Vec<_> = text.split('\n').collect();
        let size = (8..=a.size)
            .rev()
            .find(|s| {
                lines
                    .iter()
                    .all(|line| text_size(*s as f32, font, line).0 <= 300)
                    && lines.len() as u32 * (*s + 5) <= ch - 20
            })
            .context("Message too long; add line breaks or shorten it")?;
        let total = lines.len() as i32 * (size as i32 + 5);
        for (i, line) in lines.iter().enumerate() {
            centered(
                &mut im,
                font,
                line,
                size,
                160,
                (ch as i32 - total) / 2 + i as i32 * (size as i32 + 5) + size as i32 / 2,
                a.color,
            );
        }
    }
    if a.clock {
        centered(
            &mut im,
            font.as_ref().unwrap(),
            &Local::now().format("%Y-%m-%d  %H:%M:%S").to_string(),
            16,
            160,
            157,
            a.color,
        );
    }
    Ok(im)
}

struct Display {
    handle: DeviceHandle<GlobalContext>,
}
impl Display {
    fn open() -> Result<Self> {
        // Enumerate explicitly so access errors are not mistaken for absent devices.
        let devices = rusb::devices()?;
        for dev in devices.iter() {
            let descriptor = dev.device_descriptor()?;
            if (descriptor.vendor_id(), descriptor.product_id()) != (0x04d9, 0xfd01) {
                continue;
            }
            let config = dev.active_config_descriptor()?;
            let valid = config.interfaces().flat_map(|i| i.descriptors()).any(|i| {
                i.interface_number() == 1
                    && i.setting_number() == 0
                    && i.endpoint_descriptors().any(|ep| {
                        ep.address() == 0x02 && ep.transfer_type() == TransferType::Interrupt
                    })
            });
            if !valid {
                bail!("Expected LCD interface 1 / interrupt OUT endpoint 0x02 missing");
            }
            let handle = dev
                .open()
                .context("Open USB LCD (try sudo; stop other panel software)")?;
            handle.set_auto_detach_kernel_driver(true)?;
            handle.claim_interface(1).context("Claim LCD interface 1")?;
            return Ok(Self { handle });
        }
        bail!("LCD 04d9:fd01 not found; run on the host with USB access")
    }
    fn send(&self, data: &Packet, timeout: Duration) -> Result<()> {
        // libusb rounds sub-millisecond timeouts to zero (infinite): clamp to 1 ms.
        let n = self
            .handle
            .write_interrupt(0x02, data, timeout.max(Duration::from_millis(1)))?;
        if n != data.len() {
            bail!("Short USB write: {n}/{}", data.len());
        }
        Ok(())
    }
    fn heartbeat(&self) -> Result<()> {
        let now = Local::now();
        self.send(
            &packet(
                [
                    0x55,
                    0xa1,
                    0xf2,
                    now.hour() as u8,
                    now.minute() as u8,
                    now.second() as u8,
                    0,
                    0,
                ],
                &[],
            ),
            Duration::from_secs(10),
        )
    }
    fn clear(&self) -> Result<()> {
        let deadline = Instant::now() + Duration::from_secs(12);
        for p in redraw(&RgbImage::new(WIDTH, HEIGHT)) {
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .context("Blackout cleanup timed out")?;
            if remaining < Duration::from_millis(1) {
                bail!("Blackout cleanup timed out");
            }
            self.send(&p, remaining.min(Duration::from_secs(1)))?;
        }
        println!("LCD cleared to black before stop.");
        Ok(())
    }
}
impl Drop for Display {
    fn drop(&mut self) {
        if let Err(e) = self.handle.release_interface(1) {
            eprintln!("USB release warning: {e}");
        }
        // rusb/libusb auto-detach reattaches the kernel driver on interface release.
    }
}
fn wait(stop: &AtomicBool, seconds: f64) -> bool {
    let end = Instant::now() + Duration::from_secs_f64(seconds);
    while !stop.load(Ordering::Relaxed) && Instant::now() < end {
        thread::sleep((end - Instant::now().min(end)).min(Duration::from_millis(50)));
    }
    stop.load(Ordering::Relaxed)
}
fn upload(d: &Display, packets: Vec<Packet>, stop: &AtomicBool) -> Result<()> {
    for p in packets {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        d.send(&p, Duration::from_secs(10))?;
    }
    Ok(())
}
fn drive(d: &Display, a: &Args, im: &mut RgbImage, stop: &AtomicBool) -> Result<()> {
    if wait(stop, 1.0) {
        return Ok(());
    }
    d.send(
        &packet([0x55, 0xa1, 0xf1, 1, 0, 0, 0, 0], &[]),
        Duration::from_secs(10),
    )?;
    d.heartbeat()?;
    upload(d, if a.partial { partial(im) } else { redraw(im) }, stop)?;
    if stop.load(Ordering::Relaxed) {
        return Ok(());
    }
    d.heartbeat()?;
    println!("Custom display uploaded; SIGTERM/Ctrl+C clears to black.");
    if a.once {
        return Ok(());
    }
    let mut next = Instant::now() + Duration::from_secs_f64(a.refresh);
    while !wait(stop, a.heartbeat) {
        d.heartbeat()?;
        if a.refresh > 0.0 && Instant::now() >= next {
            if a.clock {
                *im = render(a)?;
            }
            upload(d, partial(im), stop)?;
            if stop.load(Ordering::Relaxed) {
                break;
            }
            d.heartbeat()?;
            next = Instant::now() + Duration::from_secs_f64(a.refresh);
        }
    }
    Ok(())
}
fn run(a: Args) -> Result<()> {
    if !a.refresh.is_finite() || !(0.0..=3600.0).contains(&a.refresh) {
        bail!("--refresh must be 0..3600 seconds");
    }
    if !a.heartbeat.is_finite() || !(0.1..=5.0).contains(&a.heartbeat) {
        bail!("--heartbeat must be 0.1..5 seconds");
    }
    if a.clock && a.refresh == 0.0 && !a.once && a.preview.is_none() {
        bail!("--clock requires --refresh > 0");
    }
    let mut im = render(&a)?;
    if let Some(path) = &a.preview {
        im.save(path)?;
        println!("Saved {}", path.display());
        return Ok(());
    }
    let stop = Arc::new(AtomicBool::new(false));
    for sig in [signal_hook::consts::SIGINT, signal_hook::consts::SIGTERM] {
        signal_hook::flag::register(sig, Arc::clone(&stop))?;
    }
    let display = Display::open()?;
    let outcome = drive(&display, &a, &mut im, &stop);
    if stop.load(Ordering::Relaxed) && !a.no_blank_on_stop {
        if let Err(e) = display.clear() {
            eprintln!("Could not clear LCD on stop: {e:#}");
        }
    }
    outcome
}
fn main() {
    if let Err(e) = run(Args::parse()) {
        eprintln!("Error: {e:#}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn gradient() -> RgbImage {
        RgbImage::from_fn(WIDTH, HEIGHT, |x, y| Rgb([x as u8, y as u8, (x + y) as u8]))
    }
    fn hash(packets: &[Packet]) -> u64 {
        packets
            .iter()
            .flatten()
            .fold(0xcbf29ce484222325u64, |h, b| {
                (h ^ *b as u64).wrapping_mul(0x100000001b3)
            })
    }
    #[test]
    fn packets_match_python_reference() {
        // Golden FNV-1a checksums from Python v1.0.1 for the same gradient.
        assert_eq!(hash(&redraw(&gradient())), 0x7b1b15c85d88e359);
        assert_eq!(hash(&partial(&gradient())), 0x14f1047812583005);
    }
    #[test]
    fn full_frame_reconstruction() {
        let im = gradient();
        let packets = redraw(&im);
        let mut data = Vec::new();
        assert_eq!(packets.len(), 27);
        for (i, p) in packets.iter().enumerate() {
            assert_eq!(p[3], (i + 1) as u8);
            assert_eq!(u16::from_le_bytes([p[4], p[5]]), (i * 4096) as u16);
            let n = u16::from_le_bytes([p[6], p[7]]) as usize;
            assert_eq!(n, if i == 26 { 2304 } else { 4096 });
            data.extend_from_slice(&p[8..8 + n]);
        }
        assert_eq!(data, rgb565(&im));
    }
    #[test]
    fn partial_frame_reconstruction() {
        let im = gradient();
        let mut data = vec![0; 108800];
        let mut seen = vec![false; 54400];
        for p in partial(&im) {
            let x = u16::from_le_bytes([p[2], p[3]]) as usize;
            let y = u16::from_le_bytes([p[4], p[5]]) as usize;
            let w = p[6] as usize;
            let h = p[7] as usize;
            for row in 0..h {
                for col in 0..w {
                    let dst = (y + row) * 320 + x + col;
                    assert!(!seen[dst]);
                    seen[dst] = true;
                    let src = 8 + (row * w + col) * 2;
                    data[dst * 2..dst * 2 + 2].copy_from_slice(&p[src..src + 2]);
                }
            }
        }
        assert!(seen.into_iter().all(|v| v));
        assert_eq!(data, rgb565(&im));
    }
    #[test]
    fn black_frame_has_no_colored_pixels() {
        let packets = redraw(&RgbImage::new(WIDTH, HEIGHT));
        assert_eq!(packets.len(), 27);
        assert_eq!(packets[0][2], 0xf0);
        assert_eq!(packets[26][2], 0xf2);
        assert!(packets.iter().all(|p| p[8..].iter().all(|b| *b == 0)));
    }
    #[test]
    fn color_validation() {
        assert_eq!(color("#ff8000").unwrap(), Rgb([255, 128, 0]));
        assert!(color("bad hex").is_err());
        assert!(color("💙xx").is_err());
    }
    #[test]
    fn cli_rejects_conflicting_content() {
        assert!(Args::try_parse_from(["skull-lcd", "--blank", "--text", "x"]).is_err());
        assert!(Args::try_parse_from(["skull-lcd", "--text", "x", "--label", "y"]).is_err());
        assert!(Args::try_parse_from(["skull-lcd", "--blank", "--clock"]).is_err());
    }
    #[test]
    fn stop_wait_is_interruptible() {
        assert!(wait(&AtomicBool::new(true), 5.0));
    }
    #[test]
    fn preview_without_usb() {
        let a = Args::try_parse_from(["skull-lcd", "--text", "MeridianX", "--clock"]).unwrap();
        let im = render(&a).unwrap();
        assert_eq!(im.dimensions(), (WIDTH, HEIGHT));
        assert!(im.pixels().any(|p| *p != Rgb([0, 0, 0])));
    }
}
