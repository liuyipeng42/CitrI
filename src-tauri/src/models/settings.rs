use super::actions::{ActionItem, default_actions};
use crate::constants::{
    ACTION_WINDOW_DEFAULT_HEIGHT, ACTION_WINDOW_DEFAULT_WIDTH, clamp_action_window_size,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashSet;
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
    pub fn sanitized(mut self) -> Self {
        let defaults = Self::default();
        let profile_defaults = ApiProfile::default();
        if self.api_profiles.is_empty() {
            self.api_profiles.push(ApiProfile::default());
        }

        let mut used_ids = HashSet::new();
        self.api_profiles.truncate(20);
        for (index, profile) in self.api_profiles.iter_mut().enumerate() {
            let base = non_empty(limited(profile.id.trim(), 68), format!("api-{}", index + 1));
            let mut id = base.clone();
            let mut suffix = 2;
            while used_ids.contains(&id) {
                id = format!("{base}-{suffix}");
                suffix += 1;
            }
            used_ids.insert(id.clone());
            profile.id = id;
            profile.name = non_empty(
                limited(profile.name.trim(), 50),
                format!("API {}", index + 1),
            );
            profile.api_url = non_empty(
                limited(profile.api_url.trim(), 2000),
                profile_defaults.api_url.clone(),
            );
            profile.api_key = limited(profile.api_key.trim(), 8000);
            profile.model = non_empty(
                limited(profile.model.trim(), 200),
                profile_defaults.model.clone(),
            );
            profile.system_prompt = limited(&profile.system_prompt, 20_000);
            profile.temperature =
                finite_clamp(profile.temperature, profile_defaults.temperature, 0.0, 2.0);
            profile.max_tokens = profile.max_tokens.clamp(1, 128_000);
            if serde_json::to_vec(&profile.extra_body).map_or(true, |value| value.len() > 100_000) {
                profile.extra_body.clear();
            }
        }

        if !self
            .api_profiles
            .iter()
            .any(|profile| profile.id == self.active_api_profile_id)
        {
            self.active_api_profile_id = self.api_profiles[0].id.clone();
        }
        self.action_opacity = self.action_opacity.clamp(20, 100);
        (self.action_window_width, self.action_window_height) =
            clamp_action_window_size(self.action_window_width, self.action_window_height);
        self.font_size = self.font_size.clamp(85, 125);
        self.history_limit = self.history_limit.min(1000);
        self.custom_accent_color =
            sanitize_hex(&self.custom_accent_color, &defaults.custom_accent_color);
        self.search_url = non_empty(limited(self.search_url.trim(), 2000), defaults.search_url);
        self.filter_list = self
            .filter_list
            .into_iter()
            .map(|item| limited(item.trim().to_lowercase(), 260))
            .filter(|item| !item.is_empty())
            .take(200)
            .collect();

        self.actions.truncate(20);
        for (index, action) in self.actions.iter_mut().enumerate() {
            let fallback = defaults.actions.iter().find(|item| item.id == action.id);
            action.id = non_empty(
                limited(action.id.trim(), 80),
                format!("custom-{}", index + 1),
            );
            action.name = non_empty(
                limited(action.name.trim(), 40),
                fallback
                    .as_ref()
                    .map_or_else(|| "自定义".into(), |item| item.name.clone()),
            );
            action.icon = non_empty(
                limited(action.icon.trim(), 60),
                fallback
                    .as_ref()
                    .map_or_else(|| "sparkles".into(), |item| item.icon.clone()),
            );
            action.built_in = fallback
                .as_ref()
                .map_or(action.built_in, |item| item.built_in);
            action.prompt = limited(&action.prompt, 20_000);
            if action.prompt.is_empty()
                && let Some(fallback) = fallback
            {
                action.prompt.clone_from(&fallback.prompt);
            }
        }
        self
    }

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

fn limited(value: impl AsRef<str>, max: usize) -> String {
    value.as_ref().chars().take(max).collect()
}
fn non_empty(value: String, fallback: String) -> String {
    if value.is_empty() { fallback } else { value }
}
fn finite_clamp(value: f64, fallback: f64, min: f64, max: f64) -> f64 {
    if value.is_finite() {
        value.clamp(min, max)
    } else {
        fallback
    }
}
fn sanitize_hex(value: &str, fallback: &str) -> String {
    let value = value.trim().to_lowercase();
    if value.len() == 7
        && value.starts_with('#')
        && value[1..]
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        value
    } else {
        fallback.into()
    }
}
