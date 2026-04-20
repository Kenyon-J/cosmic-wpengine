#![cfg(test)]

use super::*;
use crate::modules::event::WeatherCondition;

/// Tests the mapping of WMO weather codes to internal `WeatherCondition` variants.
/// This prevents incorrect weather visuals from being shown for unhandled or edge-case API responses.
#[test]
fn test_wmo_code_to_condition() {
    let test_cases = vec![
        (0, WeatherCondition::Clear),
        (1, WeatherCondition::PartlyCloudy),
        (2, WeatherCondition::PartlyCloudy),
        (3, WeatherCondition::Cloudy),
        (4, WeatherCondition::PartlyCloudy),  // Default case
        (44, WeatherCondition::PartlyCloudy), // Default case
        (45, WeatherCondition::Fog),
        (46, WeatherCondition::PartlyCloudy), // Default case
        (48, WeatherCondition::Fog),
        (50, WeatherCondition::PartlyCloudy), // Default case
        (51, WeatherCondition::Rain),
        (60, WeatherCondition::Rain),
        (67, WeatherCondition::Rain),
        (68, WeatherCondition::PartlyCloudy), // Default case
        (71, WeatherCondition::Snow),
        (75, WeatherCondition::Snow),
        (77, WeatherCondition::Snow),
        (78, WeatherCondition::PartlyCloudy), // Default case
        (80, WeatherCondition::Rain),
        (82, WeatherCondition::Rain),
        (83, WeatherCondition::PartlyCloudy), // Default case
        (85, WeatherCondition::Snow),
        (86, WeatherCondition::Snow),
        (87, WeatherCondition::PartlyCloudy), // Default case
        (95, WeatherCondition::Thunderstorm),
        (99, WeatherCondition::Thunderstorm),
        (100, WeatherCondition::PartlyCloudy), // Default case
    ];

    for (code, expected) in test_cases {
        assert_eq!(
            WeatherWatcher::wmo_code_to_condition(code),
            expected,
            "WMO code {} should map to {:?}",
            code,
            expected
        );
    }
}
