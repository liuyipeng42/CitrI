use super::actions::ActionItem;
use super::settings::{ApiProfile, Settings};
use crate::constants::{MAX_HISTORY_LIMIT, clamp_action_window_size};
use std::collections::HashSet;

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
            sanitize_profile(profile, index, &profile_defaults, &mut used_ids);
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
        self.history_limit = self.history_limit.min(MAX_HISTORY_LIMIT);
        self.custom_accent_color =
            sanitize_hex(&self.custom_accent_color, &defaults.custom_accent_color);
        self.search_url = non_empty(limited(self.search_url.trim(), 2000), defaults.search_url);

        let mut seen_filters = HashSet::new();
        self.filter_list = self
            .filter_list
            .into_iter()
            .map(|item| limited(item.trim().to_lowercase(), 260))
            .filter(|item| !item.is_empty() && seen_filters.insert(item.clone()))
            .take(200)
            .collect();

        let mut used_action_ids = HashSet::new();
        self.actions.truncate(20);
        for (index, action) in self.actions.iter_mut().enumerate() {
            sanitize_action(action, index, &defaults.actions, &mut used_action_ids);
        }
        self
    }
}

fn sanitize_profile(
    profile: &mut ApiProfile,
    index: usize,
    defaults: &ApiProfile,
    used_ids: &mut HashSet<String>,
) {
    let base = non_empty(limited(profile.id.trim(), 68), format!("api-{}", index + 1));
    profile.id = unique_id(used_ids, base);
    profile.name = non_empty(
        limited(profile.name.trim(), 50),
        format!("API {}", index + 1),
    );
    profile.api_url = non_empty(
        limited(profile.api_url.trim(), 2000),
        defaults.api_url.clone(),
    );
    profile.api_key = limited(profile.api_key.trim(), 8000);
    profile.model = non_empty(limited(profile.model.trim(), 200), defaults.model.clone());
    profile.system_prompt = limited(&profile.system_prompt, 20_000);
    profile.temperature = finite_clamp(profile.temperature, defaults.temperature, 0.0, 2.0);
    profile.max_tokens = profile.max_tokens.clamp(1, 128_000);
    if serde_json::to_vec(&profile.extra_body).map_or(true, |value| value.len() > 100_000) {
        profile.extra_body.clear();
    }
}

fn sanitize_action(
    action: &mut ActionItem,
    index: usize,
    default_actions: &[ActionItem],
    used_ids: &mut HashSet<String>,
) {
    let trimmed_id = action.id.trim();
    let fallback = default_actions.iter().find(|item| item.id == trimmed_id);
    let base = non_empty(limited(trimmed_id, 80), format!("custom-{}", index + 1));
    action.id = unique_id(used_ids, base);
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
        .is_some_and(|item| item.built_in && item.id == action.id);
    action.prompt = limited(&action.prompt, 20_000);
    if action.prompt.is_empty()
        && let Some(fallback) = fallback
    {
        action.prompt.clone_from(&fallback.prompt);
    }
}

fn limited(value: impl AsRef<str>, max: usize) -> String {
    value.as_ref().chars().take(max).collect()
}

fn non_empty(value: String, fallback: String) -> String {
    if value.is_empty() { fallback } else { value }
}

fn unique_id(used: &mut HashSet<String>, base: String) -> String {
    let mut id = base.clone();
    let mut suffix = 2;
    while !used.insert(id.clone()) {
        id = format!("{base}-{suffix}");
        suffix += 1;
    }
    id
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
