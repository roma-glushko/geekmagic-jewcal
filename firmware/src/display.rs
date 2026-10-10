//! ST7789 240×240 over SPI mode 3, driven directly: no `mipidsi`, which avoids
//! the embedded-hal 0.2/1.0 split (same reasoning as Clawdmeter).
//!
//! The UI renders into a palette-indexed canvas at half resolution. `flush`
//! expands every art pixel to 2×2 panel pixels through a 16-entry RGB565
//! lookup table and streams the result in DMA-sized bands.

use esp_idf_hal::{
    delay::FreeRtos,
    gpio::{AnyInputPin, Output, OutputPin, PinDriver},
    spi::{
        config::{Config as SpiConfig, DriverConfig, MODE_3},
        Dma, SpiAnyPins, SpiDeviceDriver, SpiDriver,
    },
    units::FromValueType,
};

const SWRESET: u8 = 0x01;
const SLPOUT: u8 = 0x11;
const COLMOD: u8 = 0x3A;
const MADCTL: u8 = 0x36;
const CASET: u8 = 0x2A;
const RASET: u8 = 0x2B;
const RAMWR: u8 = 0x2C;
const INVON: u8 = 0x21;
const NORON: u8 = 0x13;
const DISPON: u8 = 0x29;

pub const WIDTH: usize = 240;
pub const HEIGHT: usize = 240;

/// Panel pixels per art pixel (the UI canvas is 120×120).
pub const SCALE: usize = 2;

/// Art rows per SPI transfer: 4 art rows = 8 panel rows × 480 B = 3,840 B,
/// under the 4 KB DMA transfer limit configured below.
const BAND_ROWS: usize = 4;
const BAND_BYTES: usize = WIDTH * 2 * SCALE * BAND_ROWS;
const DMA_MAX: usize = 4096;

const _: () = assert!(SCALE == 2, "flush() writes exactly two copies per pixel");
const _: () = assert!(BAND_BYTES <= DMA_MAX);

pub struct Display<'d> {
    spi: SpiDeviceDriver<'d, SpiDriver<'d>>,
    dc: PinDriver<'d, Output>,
    band: Box<[u8; BAND_BYTES]>,
}

impl<'d> Display<'d> {
    /// The panel survives 40 MHz on the SmallTV Pro; 80 MHz does not.
    pub fn new(
        spi2: impl SpiAnyPins + 'd,
        clk: impl OutputPin + 'd,
        mosi: impl OutputPin + 'd,
        cs: impl OutputPin + 'd,
        dc: impl OutputPin + 'd,
        rst: impl OutputPin + 'd,
    ) -> anyhow::Result<Self> {
        let mut rst_pin = PinDriver::output(rst)?;
        rst_pin.set_high()?;
        FreeRtos::delay_ms(10);
        rst_pin.set_low()?;
        FreeRtos::delay_ms(10);
        rst_pin.set_high()?;
        FreeRtos::delay_ms(120);
        // Keep RST driven high for the life of the program.
        Box::leak(Box::new(rst_pin));

        let dc_pin = PinDriver::output(dc)?;

        let spi = SpiDeviceDriver::new_single(
            spi2,
            clk,
            mosi,
            Option::<AnyInputPin>::None,
            Some(cs),
            &DriverConfig::new().dma(Dma::Auto(DMA_MAX)),
            &SpiConfig::new().baudrate(40.MHz().into()).data_mode(MODE_3),
        )?;

        let mut disp = Self {
            spi,
            dc: dc_pin,
            band: vec![0u8; BAND_BYTES].into_boxed_slice().try_into().expect("band size"),
        };
        disp.init()?;
        Ok(disp)
    }

    fn cmd(&mut self, cmd: u8) -> anyhow::Result<()> {
        self.dc.set_low()?;
        self.spi.write(&[cmd])?;
        Ok(())
    }

    fn data(&mut self, data: &[u8]) -> anyhow::Result<()> {
        self.dc.set_high()?;
        self.spi.write(data)?;
        Ok(())
    }

    fn init(&mut self) -> anyhow::Result<()> {
        self.cmd(SWRESET)?;
        FreeRtos::delay_ms(150);
        self.cmd(SLPOUT)?;
        FreeRtos::delay_ms(10);
        self.cmd(COLMOD)?;
        self.data(&[0x55])?; // 16 bit/pixel
        self.cmd(MADCTL)?;
        self.data(&[0x00])?;
        self.cmd(INVON)?; // this IPS panel needs inversion on
        self.cmd(NORON)?;
        FreeRtos::delay_ms(10);
        self.cmd(DISPON)?;
        FreeRtos::delay_ms(10);
        Ok(())
    }

    fn set_window(&mut self, x0: usize, y0: usize, x1: usize, y1: usize) -> anyhow::Result<()> {
        let (x0, y0, x1, y1) = (x0 as u16, y0 as u16, x1 as u16, y1 as u16);
        self.cmd(CASET)?;
        self.data(&[(x0 >> 8) as u8, x0 as u8, (x1 >> 8) as u8, x1 as u8])?;
        self.cmd(RASET)?;
        self.data(&[(y0 >> 8) as u8, y0 as u8, (y1 >> 8) as u8, y1 as u8])?;
        self.cmd(RAMWR)?;
        Ok(())
    }

    /// Push the art-pixel rectangle `(x, y, w, h)` of an indexed canvas
    /// (`stride` art pixels per row) to the panel, scaled 2×.
    #[allow(clippy::too_many_arguments)]
    pub fn flush(
        &mut self,
        px: &[u8],
        stride: usize,
        lut: &[u16; 16],
        x: usize,
        y: usize,
        w: usize,
        h: usize,
    ) -> anyhow::Result<()> {
        if w == 0 || h == 0 {
            return Ok(());
        }
        debug_assert!((x + w) * SCALE <= WIDTH && (y + h) * SCALE <= HEIGHT);
        self.set_window(x * SCALE, y * SCALE, (x + w) * SCALE - 1, (y + h) * SCALE - 1)?;
        self.dc.set_high()?;

        let row_bytes = w * SCALE * 2;
        let mut row = y;
        while row < y + h {
            let rows = (y + h - row).min(BAND_ROWS);
            let mut o = 0;
            for r in row..row + rows {
                let start = o;
                for &c in &px[r * stride + x..r * stride + x + w] {
                    let [hi, lo] = lut[(c & 0x0F) as usize].to_be_bytes();
                    self.band[o..o + 4].copy_from_slice(&[hi, lo, hi, lo]);
                    o += 4;
                }
                // Second panel row of this art row.
                self.band.copy_within(start..o, o);
                o += row_bytes;
            }
            self.spi.write(&self.band[..o])?;
            row += rows;
        }
        Ok(())
    }
}
