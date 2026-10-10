# Troubleshooting

## Installing

**The update page says *Not Enough Space* (SmallTV Ultra).** You uploaded `jewcal-ultra.bin` to the original
firmware. Upload `jewcal-ultra-installer.bin` first, then `jewcal-ultra.bin` from the installer's page. Nothing was
written.

**The installer's screen shows no address on my network.** The original firmware's WiFi was not saved. Join the
installer's own network, `JEWCAL-INSTALL-XXXX`, and open `http://192.168.4.1/`.

**My computer does not see the SmallTV Pro.** Try another USB cable: many only carry power. Some Pro revisions have
a power-only USB port; see [Install on the SmallTV Pro](install-pro.md).

## WiFi

**It goes back to SETUP after I enter the password.** Check the password, and that the network is 2.4 GHz: the
SmallTV cannot see 5 GHz-only networks. JewCal also returns to SETUP by itself when it cannot join the saved network
three times in a row.

**I changed my WiFi network.** Use **Forget WiFi** at the bottom of the settings page (Ultra), or hold the top of the
case for 8 seconds (Pro). JewCal restarts into SETUP.

**`http://jewcal.local/` does not open.** Some Android phones and some networks do not resolve `.local` names. Use
the address on JewCal's info screen, which ends every round of screens.

## The calendar

**SET LOCATION.** Open the settings page and choose your city.

**"hebcal.com does not know this city".** Pick a larger town nearby, or enter coordinates under *or coordinates*.

**NO CALENDAR, with an error.** hebcal.com could not be reached. JewCal retries by itself, waiting up to an hour
between tries. The SmallTV Ultra has no room to keep the old calendar while it loads a new one, so a failed nightly
refresh shows this until the next try succeeds.

**A red pixel in the corner.** The calendar has not been refreshed for over 36 hours. The times shown are still right.

**No weather.** api.open-meteo.com could not be reached. JewCal tries again every 10 minutes and hides weather older
than 3 hours.

**The times differ from my calendar.** Check the candle-lighting and havdalah choices in [Settings](settings.md#times),
and that the right city is selected. All times are Hebcal's for that location.

## The screen

**The screen stays black, but the settings page opens (SmallTV Ultra).** Versions before 0.1.0 did not light the
screen after a power cut. [Update JewCal](install-ultra.md#updating-jewcal).

**Red and blue look swapped.** Some panels order their colors differently. Please
[open an issue](https://github.com/roma-glushko/geekmagic-jewcal/issues) with a photo.

**The screen is too bright at night, or too dim.** Adjust the brightness and the night hours in
[Settings](settings.md#display).

## Safe mode

**SAFE MODE (SmallTV Ultra).** JewCal crashed three times in a row, so it started without the calendar to keep the
update page reachable. Open the address on the screen; the page shows the last error.

- **Restart JewCal** tries again.
- **Upload a firmware** installs a fixed version, or GeekMagic's original firmware.

Please [report the error](https://github.com/roma-glushko/geekmagic-jewcal/issues) shown there.

**The screen shows nothing at all, and there is no page.** A SmallTV Ultra that does not start can only be rewritten
through its serial pads inside the case (a 3.3 V USB-serial adapter). The steps are in the release zip's
`INSTALL.txt`.
