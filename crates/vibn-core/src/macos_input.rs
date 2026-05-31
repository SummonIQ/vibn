//! Native macOS input synthesis via CGEvent. Used by the computer-use
//! tools (mouse_click / mouse_move / mouse_drag / scroll / cursor_position).
//!
//! Why native instead of AppleScript: `tell application "System Events" to
//! click` is slow, can't target arbitrary coordinates, and can't synthesise
//! scroll events. CGEvent is what the OS uses internally, so it's both
//! faster and accurate to the pixel.
//!
//! Permissions: synthetic mouse events require the parent app to be granted
//! Accessibility (System Settings → Privacy → Accessibility). The OS will
//! silently swallow events if the permission is missing, so callers should
//! pair these calls with check_desktop_permissions in the UI.

#![cfg(target_os = "macos")]

use core_graphics::event::{
    CGEvent, CGEventTapLocation, CGEventType, CGMouseButton, EventField, ScrollEventUnit,
};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use core_graphics::geometry::CGPoint;

const TAP: CGEventTapLocation = CGEventTapLocation::HID;

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrusted() -> bool;
}

/// Check whether the current process has Accessibility permission. CGEvent
/// posts are silently dropped without it, so we want to fail loudly with a
/// pointer to System Settings instead of pretending the click landed.
pub fn accessibility_granted() -> bool {
    unsafe { AXIsProcessTrusted() }
}

fn require_accessibility() -> Result<(), String> {
    if accessibility_granted() {
        Ok(())
    } else {
        Err("accessibility_denied: macOS won't deliver synthesised mouse/keyboard events to this app. Grant access in System Settings → Privacy & Security → Accessibility, then try again.".into())
    }
}

fn source() -> Result<CGEventSource, String> {
    CGEventSource::new(CGEventSourceStateID::CombinedSessionState)
        .map_err(|_| "failed to create CGEventSource".to_owned())
}

/// Read the current cursor position in screen coordinates (origin top-left).
pub fn cursor_position() -> Result<(f64, f64), String> {
    let evt = CGEvent::new(source()?).map_err(|_| "CGEvent::new failed".to_owned())?;
    let p = evt.location();
    Ok((p.x, p.y))
}

pub fn mouse_move(x: f64, y: f64) -> Result<(), String> {
    require_accessibility()?;
    let src = source()?;
    let evt = CGEvent::new_mouse_event(
        src,
        CGEventType::MouseMoved,
        CGPoint::new(x, y),
        CGMouseButton::Left,
    )
    .map_err(|_| "CGEvent::new_mouse_event(MouseMoved) failed".to_owned())?;
    evt.post(TAP);
    Ok(())
}

#[derive(Debug, Clone, Copy)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

impl MouseButton {
    fn cg(self) -> CGMouseButton {
        match self {
            MouseButton::Left => CGMouseButton::Left,
            MouseButton::Right => CGMouseButton::Right,
            MouseButton::Middle => CGMouseButton::Center,
        }
    }
    fn down(self) -> CGEventType {
        match self {
            MouseButton::Left => CGEventType::LeftMouseDown,
            MouseButton::Right => CGEventType::RightMouseDown,
            MouseButton::Middle => CGEventType::OtherMouseDown,
        }
    }
    fn up(self) -> CGEventType {
        match self {
            MouseButton::Left => CGEventType::LeftMouseUp,
            MouseButton::Right => CGEventType::RightMouseUp,
            MouseButton::Middle => CGEventType::OtherMouseUp,
        }
    }
}

/// Click at (x, y). `clicks` of 2 yields a double-click, 3 a triple-click, etc.
pub fn mouse_click(x: f64, y: f64, button: MouseButton, clicks: u32) -> Result<(), String> {
    require_accessibility()?;
    let clicks = clicks.max(1) as i64;
    let pt = CGPoint::new(x, y);

    // Move first so the down/up land where the model asked, even if some
    // other process nudged the cursor.
    let mv = CGEvent::new_mouse_event(source()?, CGEventType::MouseMoved, pt, button.cg())
        .map_err(|_| "MouseMoved event failed".to_owned())?;
    mv.post(TAP);

    for n in 1..=clicks {
        let down = CGEvent::new_mouse_event(source()?, button.down(), pt, button.cg())
            .map_err(|_| "mouse-down event failed".to_owned())?;
        down.set_integer_value_field(EventField::MOUSE_EVENT_CLICK_STATE, n);
        down.post(TAP);

        let up = CGEvent::new_mouse_event(source()?, button.up(), pt, button.cg())
            .map_err(|_| "mouse-up event failed".to_owned())?;
        up.set_integer_value_field(EventField::MOUSE_EVENT_CLICK_STATE, n);
        up.post(TAP);
    }
    Ok(())
}

/// Press at (from_x, from_y), drag to (to_x, to_y), release. Used for things
/// like text selection and slider manipulation.
pub fn mouse_drag(
    from_x: f64,
    from_y: f64,
    to_x: f64,
    to_y: f64,
    button: MouseButton,
) -> Result<(), String> {
    require_accessibility()?;
    let start = CGPoint::new(from_x, from_y);
    let end = CGPoint::new(to_x, to_y);

    let mv = CGEvent::new_mouse_event(source()?, CGEventType::MouseMoved, start, button.cg())
        .map_err(|_| "MouseMoved event failed".to_owned())?;
    mv.post(TAP);

    let down = CGEvent::new_mouse_event(source()?, button.down(), start, button.cg())
        .map_err(|_| "mouse-down event failed".to_owned())?;
    down.post(TAP);

    let drag_type = match button {
        MouseButton::Left => CGEventType::LeftMouseDragged,
        MouseButton::Right => CGEventType::RightMouseDragged,
        MouseButton::Middle => CGEventType::OtherMouseDragged,
    };
    let drag = CGEvent::new_mouse_event(source()?, drag_type, end, button.cg())
        .map_err(|_| "mouse-drag event failed".to_owned())?;
    drag.post(TAP);

    let up = CGEvent::new_mouse_event(source()?, button.up(), end, button.cg())
        .map_err(|_| "mouse-up event failed".to_owned())?;
    up.post(TAP);
    Ok(())
}

/// Scroll by `dy` "lines" vertically (positive = scroll up, the OS's
/// natural-scroll convention). `dx` is horizontal.
pub fn scroll(dx: i32, dy: i32) -> Result<(), String> {
    require_accessibility()?;
    // wheel1 = vertical, wheel2 = horizontal. wheel_count=2 reports both axes.
    let evt = CGEvent::new_scroll_event(source()?, ScrollEventUnit::LINE, 2, dy, dx, 0)
        .map_err(|_| "CGEvent::new_scroll_event failed".to_owned())?;
    evt.post(TAP);
    Ok(())
}
