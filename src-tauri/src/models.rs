use crate::constants::{
    ACTION_WINDOW_DEFAULT_HEIGHT, ACTION_WINDOW_DEFAULT_WIDTH, clamp_action_window_size,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashSet;
use std::time::{Duration, Instant};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct ActionItem {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub enabled: bool,
    pub built_in: bool,
    pub prompt: String,
}

impl Default for ActionItem {
    fn default() -> Self {
        Self {
            id: "custom".into(),
            name: "自定义".into(),
            icon: "sparkles".into(),
            enabled: true,
            built_in: false,
            prompt: String::new(),
        }
    }
}

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

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SelectionPayload {
    pub text: String,
    pub x: i32,
    pub y: i32,
    pub app_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ActionWindowSession {
    pub action: ActionItem,
    pub selected_text: String,
    pub pinned: bool,
    pub frontend_ready: bool,
    pub auto_close_armed: bool,
    pub last_window_size: Option<(u32, u32)>,
    pub(crate) native_interaction_until: Option<Instant>,
}

impl ActionWindowSession {
    pub fn mark_frontend_ready(&mut self, focused: bool) {
        self.frontend_ready = true;
        self.auto_close_armed = focused;
    }

    pub fn on_focus_changed(&mut self, focused: bool) -> bool {
        if focused {
            self.native_interaction_until = None;
            if self.frontend_ready {
                self.auto_close_armed = true;
            }
            return false;
        }
        self.auto_close_armed && !self.pinned
    }

    pub fn should_close_after_blur(&self, focused: bool, cursor_near_window: bool) -> bool {
        !focused
            && !cursor_near_window
            && !self.native_interaction_is_active()
            && self.auto_close_armed
            && !self.pinned
    }

    pub fn begin_native_interaction(&mut self) {
        self.native_interaction_until = Some(Instant::now() + Duration::from_secs(2));
    }

    pub fn note_native_window_change(&mut self) {
        self.native_interaction_until = Some(Instant::now() + Duration::from_millis(500));
    }

    pub fn note_window_size(&mut self, width: u32, height: u32) {
        self.last_window_size = Some((width, height));
        self.note_native_window_change();
    }

    fn native_interaction_is_active(&self) -> bool {
        self.native_interaction_until
            .is_some_and(|deadline| Instant::now() <= deadline)
    }
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ActionSession {
    pub action: ActionItem,
    pub selected_text: String,
    pub settings: PublicSettings,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ToolbarState {
    pub settings: PublicSettings,
    pub selection: Option<SelectionPayload>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PlatformInfo {
    pub platform: String,
    pub display_protocol: String,
}

pub fn default_actions() -> Vec<ActionItem> {
    vec![
        action("copy", "复制", "copy", true, ""),
        action("search", "搜索", "search", true, ""),
        action(
            "translate",
            "翻译",
            "languages",
            true,
            r#"你是一个划词翻译工具。请判断输入文本的语言和文本类型，并严格按照以下规则处理。

## 处理规则

1. **非中文单词**

   * 使用简体中文解释。
   * 给出词性和常见释义。
   * 如果存在多个常见词性或含义，分别列出。
   * 仅在有必要时简要说明不同词性、含义或用法之间的区别。
   * 不提供例句，除非理解词义必须依赖例句。

2. **中文单词**

   * 给出常见英文译法及对应词性。
   * 如果存在多个常见译法，分别列出。
   * 仅在有必要时简要说明不同译法在语义、语气或使用场景上的区别。

3. **非中文短语、句子或段落**

   * 翻译为自然、准确、符合中文表达习惯的简体中文。
   * 不逐词翻译，不额外解释。

4. **中文短语、句子或段落**

   * 翻译为自然、准确、地道的英文。
   * 不逐词解释，不额外补充内容。

5. **通用要求**

   * 保留原文的语气、含义、专有名词、数字、符号以及必要格式。
   * 根据上下文选择最自然、最常用的含义，不罗列生僻或明显不相关的释义。
   * 输入中同时包含中英文时，根据整体语义判断主要翻译方向；专有名词、品牌名、型号、代码等无需强行翻译。
   * 不解释判断过程或翻译过程。
   * 不添加前言、总结、提示、免责声明或其他无关内容。
   * 不使用 Markdown 表格。
   * 严格按照下方规定的输出格式输出。

## 输出格式

### A. 非中文单词

存在一个词性时：

**词性**：释义1；释义2

存在多个词性时：

**词性1**：释义1；释义2
**词性2**：释义1；释义2

如确有必要补充用法区别：

**用法**：简短说明

示例格式：

**名词**：应用；申请
**动词**：应用；运用
**用法**：表示“申请”时通常用作名词，表示“应用某种方法”时可用作动词。

### B. 中文单词

存在一个常见译法时：

**英文**：translation
**词性**：词性

存在多个常见译法时：

**英文**：translation 1（词性）
**英文**：translation 2（词性）

如确有必要补充区别：

**区别**：简短说明

示例格式：

**英文**：issue（名词）
**英文**：problem（名词）
**区别**：issue 较中性，可指需要讨论或处理的事项；problem 更强调实际存在的问题或困难。

### C. 短语、句子或段落

只输出翻译结果本身，不添加：

* “翻译：”
* “译文：”
* 词性说明
* 原文复述
* 引号
* 解释或备注

如果原文包含多个段落、列表或换行，尽量保留对应结构。

## 待处理文本

{{text}}"#,
        ),
        action(
            "summary",
            "总结",
            "align-left",
            true,
            "请用简洁的中文总结下面的内容，保留关键事实和结论：\n\n{{text}}",
        ),
        action(
            "explain",
            "解释",
            "message-circle-question",
            true,
            "请用清晰、易懂的中文解释下面的内容，必要时补充背景和例子：\n\n{{text}}",
        ),
        action(
            "refine",
            "润色",
            "sparkles",
            false,
            "在不改变原意的前提下，润色下面的文本，使其更流畅、准确、自然。只输出润色后的文本：\n\n{{text}}",
        ),
    ]
}

fn action(id: &str, name: &str, icon: &str, enabled: bool, prompt: &str) -> ActionItem {
    ActionItem {
        id: id.into(),
        name: name.into(),
        icon: icon.into(),
        enabled,
        built_in: true,
        prompt: prompt.into(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_ranges_and_active_profile() {
        let mut value = Settings {
            action_opacity: 1,
            action_window_width: 100,
            action_window_height: 5000,
            font_size: 250,
            ..Settings::default()
        };
        value.api_profiles.push(ApiProfile {
            id: "second".into(),
            api_url: " https://example.com/v1 ".into(),
            ..ApiProfile::default()
        });
        value.active_api_profile_id = "second".into();
        let value = value.sanitized();
        assert_eq!(value.action_opacity, 20);
        assert_eq!(value.action_window_width, 300);
        assert_eq!(value.action_window_height, 2160);
        assert_eq!(value.font_size, 125);
        assert_eq!(value.accent_color, AccentColor::Orange);
        assert_eq!(value.filter_mode, FilterMode::Default);
        assert_eq!(value.active_profile().api_url, "https://example.com/v1");
    }

    #[test]
    fn default_translation_prompt_uses_the_structured_format() {
        let translate = default_actions()
            .into_iter()
            .find(|action| action.id == "translate")
            .expect("the default translation action must exist");

        assert!(translate.prompt.starts_with("你是一个划词翻译工具。"));
        assert!(translate.prompt.contains("## 处理规则"));
        assert!(translate.prompt.contains("## 输出格式"));
        assert!(translate.prompt.ends_with("## 待处理文本\n\n{{text}}"));
    }

    #[test]
    fn preserves_valid_custom_color() {
        let value = Settings {
            accent_color: AccentColor::Custom,
            custom_accent_color: "#12AbEf".into(),
            ..Settings::default()
        }
        .sanitized();
        assert_eq!(value.custom_accent_color, "#12abef");
    }

    #[test]
    fn arms_auto_close_only_after_a_ready_window_has_focus() {
        let mut session = ActionWindowSession {
            action: ActionItem::default(),
            selected_text: String::new(),
            pinned: false,
            frontend_ready: false,
            auto_close_armed: false,
            last_window_size: None,
            native_interaction_until: None,
        };

        assert!(!session.on_focus_changed(false));
        assert!(!session.on_focus_changed(true));
        session.mark_frontend_ready(false);
        assert!(!session.on_focus_changed(false));
        assert!(!session.on_focus_changed(true));
        assert!(session.on_focus_changed(false));

        session.pinned = true;
        assert!(!session.on_focus_changed(false));
    }

    #[test]
    fn delayed_blur_check_observes_native_interaction_state() {
        let mut session = ActionWindowSession {
            action: ActionItem::default(),
            selected_text: String::new(),
            pinned: false,
            frontend_ready: true,
            auto_close_armed: true,
            last_window_size: None,
            native_interaction_until: None,
        };

        session.begin_native_interaction();
        assert!(session.on_focus_changed(false));
        assert!(!session.should_close_after_blur(false, true));
        assert!(!session.should_close_after_blur(false, false));

        session.native_interaction_until = None;
        assert!(!session.should_close_after_blur(false, true));
        assert!(session.should_close_after_blur(false, false));
        assert!(!session.should_close_after_blur(true, false));
    }

    #[test]
    fn resize_event_can_cancel_an_already_scheduled_close() {
        let mut session = ActionWindowSession {
            action: ActionItem::default(),
            selected_text: String::new(),
            pinned: false,
            frontend_ready: true,
            auto_close_armed: true,
            last_window_size: None,
            native_interaction_until: None,
        };

        assert!(session.on_focus_changed(false));
        session.note_window_size(720, 560);
        assert!(!session.should_close_after_blur(false, false));
    }

    #[test]
    fn records_the_latest_logical_window_size() {
        let mut session = ActionWindowSession {
            action: ActionItem::default(),
            selected_text: String::new(),
            pinned: false,
            frontend_ready: true,
            auto_close_armed: true,
            last_window_size: None,
            native_interaction_until: None,
        };

        session.note_window_size(720, 560);
        assert_eq!(session.last_window_size, Some((720, 560)));
    }
}
