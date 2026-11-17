use super::actions::{ActionItem, default_actions};
use crate::constants::{ACTION_WINDOW_DEFAULT_HEIGHT, ACTION_WINDOW_DEFAULT_WIDTH};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct ApiProfile {
    pub id: String,
    pub name: String,
    pub api_url: String,
    pub api_key: String,
    pub model: String,
    pub system_prompt: String,
    pub temperature: f64,
    pub max_tokens: u32,
    pub extra_body: Map<String, Value>,
}

impl Default for ApiProfile {
    fn default() -> Self {
        Self {
            id: "api-1".into(),
            name: "默认 API".into(),
            api_url: "https://api.openai.com/v1".into(),
            api_key: String::new(),
            model: "gpt-4.1-mini".into(),
            system_prompt: "你是一个准确、简洁、可靠的划词助手。".into(),
            temperature: 0.3,
            max_tokens: 2048,
            extra_body: Map::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS, Default)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum AccentColor {
    #[default]
    Orange,
    Cherry,
    Blue,
    Green,
    Purple,
    Custom,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, TS, Default)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum FilterMode {
    #[default]
    Default,
    Whitelist,
    Blacklist,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct Settings {
    pub enabled: bool,
    pub api_profiles: Vec<ApiProfile>,
    pub active_api_profile_id: String,
    pub compact: bool,
    pub follow_toolbar: bool,
    pub auto_close: bool,
    pub auto_pin: bool,
    pub remember_action_window_size: bool,
    pub action_window_width: u32,
    pub action_window_height: u32,
    pub action_opacity: u8,
    pub font_size: u8,
    pub history_limit: u32,
    pub accent_color: AccentColor,
    pub custom_accent_color: String,
    pub search_url: String,
    pub filter_mode: FilterMode,
    pub filter_list: Vec<String>,
    pub actions: Vec<ActionItem>,
}

impl Default for Settings {
    fn default() -> Self {
        let profile = ApiProfile::default();
        Self {
            enabled: true,
            active_api_profile_id: profile.id.clone(),
            api_profiles: vec![profile],
            compact: false,
            follow_toolbar: true,
            auto_close: true,
            auto_pin: false,
            remember_action_window_size: false,
            action_window_width: ACTION_WINDOW_DEFAULT_WIDTH,
            action_window_height: ACTION_WINDOW_DEFAULT_HEIGHT,
            action_opacity: 100,
            font_size: 100,
            history_limit: 100,
            accent_color: AccentColor::Orange,
            custom_accent_color: "#f97316".into(),
            search_url: "https://www.google.com/search?q={{queryString}}".into(),
            filter_mode: FilterMode::Default,
            filter_list: Vec::new(),
            actions: default_actions(),
        }
    }
}

impl Settings {
    pub fn active_profile(&self) -> ApiProfile {
        self.api_profiles
            .iter()
            .find(|profile| profile.id == self.active_api_profile_id)
            .cloned()
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PublicAction {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub enabled: bool,
    pub built_in: bool,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PublicSettings {
    pub compact: bool,
    pub auto_close: bool,
    pub auto_pin: bool,
    pub action_opacity: u8,
    pub font_size: u8,
    pub accent_color: AccentColor,
    pub custom_accent_color: String,
    pub actions: Vec<PublicAction>,
}

impl From<&Settings> for PublicSettings {
    fn from(settings: &Settings) -> Self {
        Self {
            compact: settings.compact,
            auto_close: settings.auto_close,
            auto_pin: settings.auto_pin,
            action_opacity: settings.action_opacity,
            font_size: settings.font_size,
            accent_color: settings.accent_color,
            custom_accent_color: settings.custom_accent_color.clone(),
            actions: settings
                .actions
                .iter()
                .map(|action| PublicAction {
                    id: action.id.clone(),
                    name: action.name.clone(),
                    icon: action.icon.clone(),
                    enabled: action.enabled,
                    built_in: action.built_in,
                })
                .collect(),
        }
    }
}
