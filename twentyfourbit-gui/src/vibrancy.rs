//! macOS window vibrancy — real frosted glass, not transparency.
//!
//! Making the window merely transparent would show the desktop sharply through
//! the interface, which is unreadable. What "glass" means on macOS is an
//! `NSVisualEffectView` placed behind the content: the window server samples
//! what is behind the window and blurs it, and the app draws semi-transparent
//! surfaces over the result.
//!
//! Everything here is best-effort. If any step fails the caller keeps an opaque
//! interface, which is a perfectly good outcome; a half-applied transparency
//! would be far worse than none.

/// Whether the platform can provide a blurred backdrop.
pub const SUPPORTED: bool = cfg!(target_os = "macos");

/// Install the blurred backdrop behind `raw`.
///
/// Returns `true` when the effect was installed, which tells the caller it may
/// now use translucent surface colours.
#[cfg(target_os = "macos")]
pub fn install(raw: raw_window_handle::RawWindowHandle) -> bool {
    use objc2::{rc::Retained, MainThreadMarker};
    use objc2_app_kit::{
        NSAutoresizingMaskOptions, NSColor, NSVisualEffectBlendingMode, NSVisualEffectMaterial,
        NSVisualEffectState, NSVisualEffectView, NSWindowOrderingMode, NSView,
    };
    use objc2_foundation::NSRect;

    let Some(mtm) = MainThreadMarker::new() else {
        tracing::warn!("vibrancy: not on the main thread");
        return false;
    };

    let raw_window_handle::RawWindowHandle::AppKit(handle) = raw else {
        return false;
    };

    // SAFETY: eframe hands us a live NSView for the duration of the run loop,
    // and we only read its geometry and attach a subview.
    unsafe {
        let view: &NSView = &*(handle.ns_view.as_ptr() as *const NSView);

        let Some(window) = view.window() else {
            tracing::warn!("vibrancy: the view has no window yet");
            return false;
        };

        // A transparent window that is *not* opaque is what lets the effect
        // view show through.
        window.setOpaque(false);
        window.setBackgroundColor(Some(&NSColor::clearColor()));

        let bounds: NSRect = view.bounds();
        let effect = NSVisualEffectView::initWithFrame(
            mtm.alloc::<NSVisualEffectView>(),
            bounds,
        );

        effect.setMaterial(NSVisualEffectMaterial::UnderWindowBackground);
        effect.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
        effect.setState(NSVisualEffectState::Active);
        effect.setAutoresizingMask(
            NSAutoresizingMaskOptions::ViewWidthSizable
                | NSAutoresizingMaskOptions::ViewHeightSizable,
        );

        // Behind the OpenGL surface, so our drawing lands on top of the blur.
        view.addSubview_positioned_relativeTo(
            &effect,
            NSWindowOrderingMode::Below,
            None,
        );

        // Keep the effect view alive for the life of the process; AppKit does
        // not take ownership of a subview we allocated.
        let _: Retained<NSVisualEffectView> = effect;

        true
    }
}

/// Non-macOS builds simply have no effect to install.
#[cfg(not(target_os = "macos"))]
pub fn install(_raw: raw_window_handle::RawWindowHandle) -> bool {
    false
}
