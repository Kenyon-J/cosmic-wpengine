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
        info!("Weather watcher started. Initial state: {}", if config.enabled { "enabled" } else { "disabled" });

        let mut last_config = config.clone();
        let mut last_fetch = tokio::time::Instant::now() - tokio::time::Duration::from_secs(86400);

        loop {
            let current_config = match super::config::Config::load_or_default() {
                Ok(c) => c.weather,
                Err(_) => last_config.clone(),
            };

            let mut force_fetch = false;
            if (current_config.enabled && !last_config.enabled) || current_config.latitude != last_config.latitude || current_config.longitude != last_config.longitude {
                force_fetch = true;
            }

            if current_config.enabled {
                let poll_interval = tokio::time::Duration::from_secs(current_config.poll_interval_minutes.max(1) * 60);
                if force_fetch || last_fetch.elapsed() >= poll_interval {
                    match Self::fetch_weather(&current_config).await {
                        Ok(data) => {
                            info!("Weather updated: {:?} {:.1}°C", data.condition, data.temperature_celsius);
                            let _ = tx.send(Event::WeatherUpdated(data)).await;
                            last_fetch = tokio::time::Instant::now();
                        }
                        Err(e) => {
                            warn!("Weather fetch failed: {}", e);
                            last_fetch = tokio::time::Instant::now() - poll_interval + tokio::time::Duration::from_secs(60);
                        }
                    }
                }
            }

            last_config = current_config;
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        }
    }

    async fn fetch_weather(config: &WeatherConfig) -> Result<WeatherData> {
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
        })
    }

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

#[derive(Deserialize)]
struct OpenMeteoResponse {
    current: CurrentWeather,
}

#[derive(Deserialize)]
struct CurrentWeather {
    temperature_2m: f32,
    weather_code: u32,
}
