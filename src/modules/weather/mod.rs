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
    pub async fn run(
        tx: Sender<Event>,
        mut config_rx: tokio::sync::watch::Receiver<super::config::Config>,
    ) -> Result<()> {
        let mut last_config = config_rx.borrow().weather.clone();
        info!(
            "Weather watcher started. Initial state: {}",
            if last_config.enabled {
                "enabled"
            } else {
                "disabled"
            }
        );

        let client = reqwest::Client::builder()
            .user_agent("cosmic-wallpaper/1.0")
            .timeout(std::time::Duration::from_secs(10))
            .build()?;

        let mut last_fetch = tokio::time::Instant::now() - tokio::time::Duration::from_secs(86400);

        loop {
            let current_config = config_rx.borrow().weather.clone();

            let mut force_fetch = false;
            if (current_config.enabled && !last_config.enabled)
                || current_config.latitude != last_config.latitude
                || current_config.longitude != last_config.longitude
            {
                force_fetch = true;
            }

            let poll_interval =
                tokio::time::Duration::from_secs(current_config.poll_interval_minutes.max(1) * 60);

            if current_config.enabled && (force_fetch || last_fetch.elapsed() >= poll_interval) {
                match Self::fetch_weather(&current_config, &client).await {
                    Ok(data) => {
                        info!(
                            "Weather updated: {:?} {:.1}°C",
                            data.condition, data.temperature_celsius
                        );
                        let _ = tx.send(Event::WeatherUpdated(Box::new(data))).await;
                        last_fetch = tokio::time::Instant::now();
                    }
                    Err(e) => {
                        warn!("Weather fetch failed: {}", e);
                        last_fetch = tokio::time::Instant::now() - poll_interval
                            + tokio::time::Duration::from_secs(60);
                    }
                }
            }

            last_config = current_config;

            let sleep_duration = if last_config.enabled {
                let elapsed = last_fetch.elapsed();
                if elapsed < poll_interval {
                    poll_interval - elapsed
                } else {
                    tokio::time::Duration::from_secs(0)
                }
            } else {
                tokio::time::Duration::from_secs(86400) // Sleep indefinitely if disabled
            };

            tokio::select! {
                _ = tokio::time::sleep(sleep_duration) => {}
                res = config_rx.changed() => {
                    if res.is_err() {
                        break; // Channel closed, time to exit
                    }
                }
            }
        }

        Ok(())
    }

    async fn fetch_weather(
        config: &WeatherConfig,
        client: &reqwest::Client,
    ) -> Result<WeatherData> {
        let response: OpenMeteoResponse = client
            .get("https://api.open-meteo.com/v1/forecast")
            .query(&[
                ("latitude", config.latitude.to_string().as_str()),
                ("longitude", config.longitude.to_string().as_str()),
                ("current", "temperature_2m,weather_code"),
            ])
            .send()
            .await?
            .json()
            .await?;

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

#[cfg(test)]
mod tests;
