//! State shared between the render loop, the fetch thread and the web server.

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use esp_idf_svc::nvs::EspDefaultNvsPartition;
use jewcal_model::calendar::CalendarModel;
use jewcal_model::weather::Weather;
use jewcal_ui::Canvas;

use crate::settings::Settings;

/// A calendar together with the settings key it was fetched under.
pub struct Loaded {
    pub key: String,
    pub model: CalendarModel,
}

#[derive(Clone, Debug, Default)]
pub struct NetStatus {
    pub ip: Option<String>,
    /// Unix time of the last successful fetch.
    pub last_fetch: Option<i64>,
    pub error: Option<String>,
    pub fetching: bool,
}

pub struct Shared {
    pub nvs: EspDefaultNvsPartition,
    pub settings: Mutex<Settings>,
    pub calendar: Mutex<Option<Arc<Loaded>>>,
    pub weather: Mutex<Option<Arc<Weather>>>,
    pub status: Mutex<NetStatus>,
    /// The frame currently on the panel, for `/screen.bmp`.
    pub frame: Mutex<Canvas>,
    /// Set by the settings page to make the fetch thread run now.
    pub wake_fetch: AtomicBool,
}

impl Shared {
    pub fn settings(&self) -> Settings {
        self.settings.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
}
