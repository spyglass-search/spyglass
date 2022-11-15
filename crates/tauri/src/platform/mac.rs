use cocoa::appkit::{NSApp, NSApplication, NSEvent, NSEventMask, NSEventSubtype};
use cocoa::base::nil;
use cocoa::foundation::{NSAutoreleasePool, NSDate, NSString};
use tauri::Window;

use crate::window;

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
