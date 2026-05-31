//! macOS-only window chrome tweaks (traffic-light position).
//!
//! Repositions the three standard window buttons on the NSWindow so they
//! vertically align with our custom titlebar contents.

use objc2::msg_send;
use objc2::runtime::AnyObject;
use objc2_app_kit::{NSWindow, NSWindowButton};
use objc2_foundation::{NSPoint, NSRect};
use tauri::WebviewWindow;

pub fn hide_native_traffic_lights(window: &WebviewWindow) {
    let ns_window_ptr = match window.ns_window() {
        Ok(ptr) => ptr as *mut AnyObject,
        Err(_) => return,
    };
    if ns_window_ptr.is_null() {
        return;
    }
    unsafe {
        let ns_window = &*(ns_window_ptr as *const NSWindow);
        for btn in [
            NSWindowButton::CloseButton,
            NSWindowButton::MiniaturizeButton,
            NSWindowButton::ZoomButton,
        ] {
            if let Some(button) = ns_window.standardWindowButton(btn) {
                let _: () = msg_send![&*button, setHidden: true];
            }
        }
    }
}

#[allow(dead_code)]
pub fn set_traffic_light_inset(window: &WebviewWindow, inset_x: f64, inset_y: f64) {
    let ns_window_ptr = match window.ns_window() {
        Ok(ptr) => ptr as *mut AnyObject,
        Err(_) => return,
    };
    if ns_window_ptr.is_null() {
        return;
    }
    unsafe {
        let ns_window = &*(ns_window_ptr as *const NSWindow);
        let buttons = [
            NSWindowButton::CloseButton,
            NSWindowButton::MiniaturizeButton,
            NSWindowButton::ZoomButton,
        ];

        // NSWindow.frame is in screen coords; what we really want is the
        // content-view height, since contentView fills the whole window when
        // titleBarStyle is Overlay.
        let Some(content_view) = ns_window.contentView() else {
            return;
        };
        let content_frame: NSRect = msg_send![&*content_view, frame];
        let content_h = content_frame.size.height;

        let mut first_x: Option<f64> = None;
        for btn in buttons.iter() {
            let Some(button) = ns_window.standardWindowButton(*btn) else {
                continue;
            };
            let frame: NSRect = msg_send![&*button, frame];
            if first_x.is_none() {
                first_x = Some(frame.origin.x);
            }
            // Preserve horizontal spacing between the three buttons.
            let dx = frame.origin.x - first_x.unwrap();
            let target_x = inset_x + dx;
            // NSWindow is bottom-left origin: convert "inset_y from top" to
            // distance from bottom.
            let target_y = content_h - inset_y - frame.size.height;
            let _: () =
                msg_send![&*button, setFrameOrigin: NSPoint::new(target_x, target_y)];
        }
    }
}
