use super::input::InputActivity;
use super::*;

#[test]
fn missing_lock_screen_key_up_does_not_block_future_captures() {
    let input = InputActivity::default();
    input.note_key_press(Key::MetaLeft);
    input.note_key_press(Key::KeyL);
    // The secure desktop consumes the releases; physical keys are now up.
    input.note_pointer_activity();
    let epoch = input.snapshot();
    assert!(input.is_idle_since_with(epoch, || true));
    // A key actually held after unlocking must still block capture.
    assert!(!input.is_idle_since_with(epoch, || false));
}

#[test]
fn input_during_physical_state_query_cancels_capture() {
    let input = InputActivity::default();
    assert!(!input.is_idle_since_with(input.snapshot(), || {
        input.note_key_press(Key::ControlLeft);
        true
    }));
}
