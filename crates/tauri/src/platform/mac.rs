use cocoa::appkit::{NSApp, NSApplication, NSEvent, NSEventMask, NSEventSubtype};
use cocoa::base::nil;
use cocoa::foundation::{NSAutoreleasePool, NSDate, NSString};

use tauri::Window;

use crate::window;
use shared::event::ClientEvent;

/// Poll for dock events
pub fn poll_app_events(window: &Window) {
    unsafe {
        let _pool = NSAutoreleasePool::new(nil);
        let ns_event = NSApp().nextEventMatchingMask_untilDate_inMode_dequeue_(
            NSEventMask::NSAppKitDefinedMask.bits(),
            NSDate::distantPast(cocoa::base::nil),
            // Use custom event loop name so we don't trampled on others.
            NSString::alloc(nil).init_str("spyglassEventLoop"),
            cocoa::base::YES,
        );

        if ns_event == nil || ns_event.eventType() as u64 == 21 {
            return;
        }

        let subtype = ns_event.subtype();
        match subtype {
            NSEventSubtype::NSApplicationActivatedEventType => {
                window::show_search_bar(window);
            }
            NSEventSubtype::NSApplicationDeactivatedEventType => {
                window::hide_search_bar(window);
            }
            _ => {}
        }
    }
}

pub fn show_search_bar(window: &Window) {
    let _ = window.show();
    window::center_search_bar(window);
    let _ = window.set_focus();

    let _ = window.emit(ClientEvent::FocusWindow.as_ref(), true);
}

pub fn hide_search_bar(window: &Window) {
    let _ = window.hide();
    let _ = window.emit(ClientEvent::ClearSearch.as_ref(), true);
}
