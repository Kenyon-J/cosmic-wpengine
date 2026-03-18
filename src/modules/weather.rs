// =============================================================================
// modules/weather.rs
// =============================================================================
// Polls the Open-Meteo weather API periodically and sends WeatherUpdated
// events to the renderer.
//
// Open-Meteo (https://open-meteo.com) is completely free and requires no
// API key — perfect for an open source project like this.
//
// WMO weather codes are an international standard for weather conditions.
// We map them to our simplified WeatherCondition enum.
// =============================================================================

use anyhow::Result;
use serde::Deserialize;
use tokio::sync::mpsc::Sender;
use tracing::{info, warn};

use super::{
    config::WeatherConfig,
    event::{Event, WeatherCondition, WeatherData},
};

pub struct WeatherWatcher;

impl WeatherWatcher {
    pub async fn run(tx: Sender<Event>, config: WeatherConfig) -> Result<()> {
        if !config.enabled {
            info!("Weather integration disabled in config");
            return Ok(());
        }

        info!(
            "Weather watcher started for ({}, {})",
            config.latitude, config.longitude
        );

        let poll_interval = tokio::time::Duration::from_secs(config.poll_interval_minutes * 60);

        loop {
            match Self::fetch_weather(&config).await {
                Ok(data) => {
                    info!(
                        "Weather updated: {:?} {:.1}°C",
                        data.condition, data.temperature_celsius
                    );
                    let _ = tx.send(Event::WeatherUpdated(data)).await;
                }
                Err(e) => {
                    warn!("Weather fetch failed: {}", e);
                }
            }

            tokio::time::sleep(poll_interval).await;
        }
    }

    async fn fetch_weather(config: &WeatherConfig) -> Result<WeatherData> {
        // Open-Meteo API — free, no key required
        // current=temperature_2m gives temperature
        // current=weather_code gives WMO weather code
        let url = format!(
            "https://api.open-meteo.com/v1/forecast?\
             latitude={}&longitude={}&\
             current=temperature_2m,weather_code",
            config.latitude, config.longitude
        );

        let response: OpenMeteoResponse = reqwest::get(&url).await?.json().await?;

        let code = response.current.weather_code;
        let temp = response.current.temperature_2m;
        let condition = Self::wmo_code_to_condition(code);

        Ok(WeatherData {
            condition,
            temperature_celsius: temp,
            location: format!("{:.2}, {:.2}", config.latitude, config.longitude),
        })
    }

    /// Map WMO weather codes to our simplified condition enum.
    /// Full WMO code table: https://open-meteo.com/en/docs#weathervariables
    fn wmo_code_to_condition(code: u32) -> WeatherCondition {
        match code {
            0 => WeatherCondition::Clear,
            1..=2 => WeatherCondition::PartlyCloudy,
            3 => WeatherCondition::Cloudy,
            45 | 48 => WeatherCondition::Fog,
            51..=67 | 80..=82 => WeatherCondition::Rain,
            71..=77 | 85..=86 => WeatherCondition::Snow,
            95..=99 => WeatherCondition::Thunderstorm,
            _ => WeatherCondition::PartlyCloudy,
        }
    }
}

// --- API response types ---
// serde automatically maps the JSON response to these structs.

#[derive(Deserialize)]
struct OpenMeteoResponse {
    current: CurrentWeather,
}

#[derive(Deserialize)]
struct CurrentWeather {
    temperature_2m: f32,
    weather_code: u32,
}
