//! `update()` handlers for the three "overlay" pages: Now Playing (lyrics
//! toggle), Visualiser (bands/smoothing), and Weather (location, poll
//! interval, temperature unit, effects).
use super::*;

impl SettingsApp {
    pub(super) fn on_toggle_show_lyrics(&mut self, state: bool) -> Task<cosmic::Action<Message>> {
        self.wp_config.audio.show_lyrics = state;
        let _ = self.wp_config.save();
        Task::none()
    }

    pub(super) fn on_bands_changed(&mut self, bands: f32) -> Task<cosmic::Action<Message>> {
        self.wp_config.audio.bands = bands as usize;
        self.schedule_debounced_save()
    }

    pub(super) fn on_smoothing_changed(&mut self, smoothing: f32) -> Task<cosmic::Action<Message>> {
        self.wp_config.audio.smoothing = smoothing;
        self.schedule_debounced_save()
    }

    pub(super) fn on_latitude_changed(&mut self, input: String) -> Task<cosmic::Action<Message>> {
        self.lat_input = input;
        if let Ok(lat) = self.lat_input.trim().parse::<f64>() {
            if (-90.0..=90.0).contains(&lat) {
                self.wp_config.weather.latitude = lat;
                return self.schedule_debounced_save();
            }
        }
        Task::none()
    }

    pub(super) fn on_longitude_changed(&mut self, input: String) -> Task<cosmic::Action<Message>> {
        self.lon_input = input;
        if let Ok(lon) = self.lon_input.trim().parse::<f64>() {
            if (-180.0..=180.0).contains(&lon) {
                self.wp_config.weather.longitude = lon;
                return self.schedule_debounced_save();
            }
        }
        Task::none()
    }

    pub(super) fn on_detect_location(&mut self) -> Task<cosmic::Action<Message>> {
        self.status_msg = fl!("status-detecting-location");
        Task::perform(fetch_ip_location(), |result| {
            Message::LocationDetected(result).into()
        })
    }

    pub(super) fn on_location_detected(
        &mut self,
        result: Result<(f64, f64), String>,
    ) -> Task<cosmic::Action<Message>> {
        match result {
            Ok((lat, lon)) => {
                self.lat_input = format!("{lat}");
                self.lon_input = format!("{lon}");
                self.wp_config.weather.latitude = lat;
                self.wp_config.weather.longitude = lon;
                let _ = self.wp_config.save();
                self.status_msg = fl!("status-location-detected");
            }
            Err(e) => {
                self.status_msg = fl!("status-could-not-detect-location", error = e.to_string());
            }
        }
        Task::none()
    }

    pub(super) fn on_poll_interval_selected(
        &mut self,
        idx: usize,
    ) -> Task<cosmic::Action<Message>> {
        if let Some(&minutes) = view::POLL_MINUTES.get(idx) {
            self.wp_config.weather.poll_interval_minutes = minutes;
            let _ = self.wp_config.save();
        }
        Task::none()
    }

    pub(super) fn on_temperature_unit_selected(
        &mut self,
        idx: usize,
    ) -> Task<cosmic::Action<Message>> {
        self.wp_config.weather.temperature_unit = if idx == 1 {
            config::TemperatureUnit::Fahrenheit
        } else {
            config::TemperatureUnit::Celsius
        };
        let _ = self.wp_config.save();
        Task::none()
    }

    pub(super) fn on_toggle_weather_enabled(
        &mut self,
        state: bool,
    ) -> Task<cosmic::Action<Message>> {
        self.wp_config.weather.enabled = state;
        let _ = self.wp_config.save();
        Task::none()
    }

    pub(super) fn on_toggle_hide_weather_effects(
        &mut self,
        state: bool,
    ) -> Task<cosmic::Action<Message>> {
        self.wp_config.weather.hide_effects = state;
        let _ = self.wp_config.save();
        Task::none()
    }
}
