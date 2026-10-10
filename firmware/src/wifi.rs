//! WiFi: station mode for normal operation, and an open access point
//! (`JEWCAL-XXXX`) for first-run setup.

use esp_idf_svc::wifi::{
    AccessPointConfiguration, AuthMethod, BlockingWifi, ClientConfiguration, Configuration, EspWifi,
};

pub type Wifi = BlockingWifi<EspWifi<'static>>;

pub fn connect(wifi: &mut Wifi, ssid: &str, pass: &str) -> anyhow::Result<()> {
    if wifi.is_started()? {
        wifi.stop()?;
    }
    wifi.set_configuration(&Configuration::Client(ClientConfiguration {
        ssid: ssid.try_into().map_err(|_| anyhow::anyhow!("SSID too long"))?,
        password: pass.try_into().map_err(|_| anyhow::anyhow!("password too long"))?,
        auth_method: if pass.is_empty() { AuthMethod::None } else { AuthMethod::WPA2Personal },
        ..Default::default()
    }))?;
    wifi.start()?;
    wifi.connect()?;
    wifi.wait_netif_up()?;
    Ok(())
}

/// Reconnect after the router went away; cheap when still connected.
pub fn ensure_connected(wifi: &mut Wifi) -> anyhow::Result<()> {
    if !wifi.is_connected()? {
        log::warn!("wifi: reconnecting");
        wifi.connect()?;
        wifi.wait_netif_up()?;
    }
    Ok(())
}

pub fn sta_ip(wifi: &Wifi) -> Option<String> {
    let ip = wifi.wifi().sta_netif().get_ip_info().ok()?.ip;
    (!ip.is_unspecified()).then(|| ip.to_string())
}

pub fn ap_name(wifi: &Wifi) -> String {
    let mac = wifi.wifi().sta_netif().get_mac().unwrap_or([0; 6]);
    format!("JEWCAL-{:02X}{:02X}", mac[4], mac[5])
}

/// Start the setup access point (with the station side on, so we can scan).
/// Returns the AP's IP and the networks in range, strongest first.
pub fn start_ap(wifi: &mut Wifi, name: &str) -> anyhow::Result<(String, Vec<String>)> {
    if wifi.is_started()? {
        wifi.stop()?;
    }
    wifi.set_configuration(&Configuration::Mixed(
        ClientConfiguration::default(),
        AccessPointConfiguration {
            ssid: name.try_into().map_err(|_| anyhow::anyhow!("AP name too long"))?,
            auth_method: AuthMethod::None,
            channel: 6,
            max_connections: 4,
            ..Default::default()
        },
    ))?;
    wifi.start()?;
    let mut seen: Vec<(i8, String)> = match wifi.scan() {
        Ok(aps) => aps.iter().filter(|a| !a.ssid.is_empty()).map(|a| (a.signal_strength, a.ssid.to_string())).collect(),
        Err(e) => {
            log::warn!("wifi scan failed: {e:?}");
            Vec::new()
        }
    };
    seen.sort_by_key(|b| std::cmp::Reverse(b.0));
    let mut names: Vec<String> = Vec::new();
    for (_, n) in seen {
        if !names.contains(&n) {
            names.push(n);
        }
    }
    let ip = wifi.wifi().ap_netif().get_ip_info()?.ip.to_string();
    Ok((ip, names))
}
