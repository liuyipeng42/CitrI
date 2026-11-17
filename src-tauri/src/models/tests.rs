use super::settings::AccentColor;
use super::*;

#[test]
fn sanitizes_ranges_and_active_profile() {
    let mut value = Settings {
        action_opacity: 1,
        action_window_width: 100,
        action_window_height: 5000,
        font_size: 250,
        filter_list: vec![" Chrome.exe ".into(), "chrome.exe".into()],
        ..Settings::default()
    };
    value.actions.push(ActionItem {
        id: "copy".into(),
        ..ActionItem::default()
    });
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
    assert_eq!(value.filter_list, vec!["chrome.exe"]);
    assert_eq!(value.actions.last().unwrap().id, "copy-2");
    assert!(!value.actions.last().unwrap().built_in);
    assert_eq!(value.active_profile().api_url, "https://example.com/v1");
}

#[test]
fn history_limit_defaults_for_older_settings_and_has_a_safe_range() {
    let legacy: Settings = serde_json::from_str("{}").unwrap();
    assert_eq!(legacy.history_limit, 100);
    assert_eq!(
        Settings {
            history_limit: u32::MAX,
            ..legacy.clone()
        }
        .sanitized()
        .history_limit,
        1000
    );
    assert_eq!(
        Settings {
            history_limit: 0,
            ..legacy
        }
        .sanitized()
        .history_limit,
        0
    );
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
