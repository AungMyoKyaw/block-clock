use gpui::Window;

pub const fn supports_always_on_top() -> bool {
    cfg!(any(target_os = "macos", target_os = "windows"))
}

#[cfg(target_os = "macos")]
pub fn set_always_on_top(window: &Window, enabled: bool) -> bool {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use std::ffi::c_void;

    unsafe extern "C" {
        fn sel_registerName(name: *const std::ffi::c_char) -> *const c_void;
        fn objc_msgSend();
    }

    let Ok(handle) = HasWindowHandle::window_handle(window) else {
        return false;
    };
    let RawWindowHandle::AppKit(appkit) = handle.as_raw() else {
        return false;
    };

    let ns_view = appkit.ns_view.as_ptr();
    if ns_view.is_null() {
        return false;
    }

    // NSFloatingWindowLevel = 3, NSNormalWindowLevel = 0.
    let level: isize = if enabled { 3 } else { 0 };

    unsafe {
        let sel_window = sel_registerName(c"window".as_ptr());
        let msg_get_window: unsafe extern "C" fn(*mut c_void, *const c_void) -> *mut c_void =
            std::mem::transmute(objc_msgSend as *const c_void);
        let ns_window = msg_get_window(ns_view, sel_window);
        if ns_window.is_null() {
            return false;
        }

        let sel_set_level = sel_registerName(c"setLevel:".as_ptr());
        let msg_set_level: unsafe extern "C" fn(*mut c_void, *const c_void, isize) =
            std::mem::transmute(objc_msgSend as *const c_void);
        msg_set_level(ns_window, sel_set_level, level);
    }

    true
}

#[cfg(target_os = "windows")]
pub fn set_always_on_top(window: &Window, enabled: bool) -> bool {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use std::ffi::c_void;

    const HWND_TOPMOST: *mut c_void = (-1isize) as *mut c_void;
    const HWND_NOTOPMOST: *mut c_void = (-2isize) as *mut c_void;
    const SWP_NOSIZE: u32 = 0x0001;
    const SWP_NOMOVE: u32 = 0x0002;
    const SWP_NOACTIVATE: u32 = 0x0010;

    #[link(name = "user32")]
    unsafe extern "system" {
        fn SetWindowPos(
            hwnd: *mut c_void,
            hwnd_insert_after: *mut c_void,
            x: i32,
            y: i32,
            cx: i32,
            cy: i32,
            flags: u32,
        ) -> i32;
    }

    let Ok(handle) = HasWindowHandle::window_handle(window) else {
        return false;
    };
    let RawWindowHandle::Win32(win32) = handle.as_raw() else {
        return false;
    };

    let hwnd = win32.hwnd.get() as *mut c_void;
    let insert_after = if enabled {
        HWND_TOPMOST
    } else {
        HWND_NOTOPMOST
    };

    unsafe {
        SetWindowPos(
            hwnd,
            insert_after,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        ) != 0
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn set_always_on_top(_window: &Window, _enabled: bool) -> bool {
    false
}

#[cfg(target_os = "macos")]
pub fn set_app_icon(icon_bytes: &[u8]) {
    use std::ffi::c_void;

    unsafe extern "C" {
        fn objc_getClass(name: *const std::ffi::c_char) -> *mut c_void;
        fn sel_registerName(name: *const std::ffi::c_char) -> *const c_void;
        fn objc_msgSend();
    }

    if icon_bytes.is_empty() {
        return;
    }

    unsafe {
        let msg_no_arg: unsafe extern "C" fn(*mut c_void, *const c_void) -> *mut c_void =
            std::mem::transmute(objc_msgSend as *const c_void);
        let msg_one_ptr: unsafe extern "C" fn(
            *mut c_void,
            *const c_void,
            *mut c_void,
        ) -> *mut c_void = std::mem::transmute(objc_msgSend as *const c_void);

        // Packaged builds already get their Dock icon from Info.plist.
        let bundle_class = objc_getClass(c"NSBundle".as_ptr());
        let string_class = objc_getClass(c"NSString".as_ptr());
        let bundle = msg_no_arg(bundle_class, sel_registerName(c"mainBundle".as_ptr()));
        let string_from_utf8: unsafe extern "C" fn(
            *mut c_void,
            *const c_void,
            *const std::ffi::c_char,
        ) -> *mut c_void = std::mem::transmute(objc_msgSend as *const c_void);
        let icon_key = string_from_utf8(
            string_class,
            sel_registerName(c"stringWithUTF8String:".as_ptr()),
            c"CFBundleIconFile".as_ptr(),
        );
        if !msg_one_ptr(
            bundle,
            sel_registerName(c"objectForInfoDictionaryKey:".as_ptr()),
            icon_key,
        )
        .is_null()
        {
            return;
        }

        let ns_data_cls = objc_getClass(c"NSData".as_ptr());
        let ns_image_cls = objc_getClass(c"NSImage".as_ptr());
        let ns_app_cls = objc_getClass(c"NSApplication".as_ptr());
        if ns_data_cls.is_null() || ns_image_cls.is_null() || ns_app_cls.is_null() {
            return;
        }

        let sel_data_with_bytes = sel_registerName(c"dataWithBytes:length:".as_ptr());
        let msg_data_with_bytes: unsafe extern "C" fn(
            *mut c_void,
            *const c_void,
            *const c_void,
            usize,
        ) -> *mut c_void = std::mem::transmute(objc_msgSend as *const c_void);
        let ns_data = msg_data_with_bytes(
            ns_data_cls,
            sel_data_with_bytes,
            icon_bytes.as_ptr().cast(),
            icon_bytes.len(),
        );
        if ns_data.is_null() {
            return;
        }

        let sel_alloc = sel_registerName(c"alloc".as_ptr());
        let image_alloc = msg_no_arg(ns_image_cls, sel_alloc);
        if image_alloc.is_null() {
            return;
        }

        let sel_init_with_data = sel_registerName(c"initWithData:".as_ptr());
        let ns_image = msg_one_ptr(image_alloc, sel_init_with_data, ns_data);
        if ns_image.is_null() {
            return;
        }

        let sel_shared_app = sel_registerName(c"sharedApplication".as_ptr());
        let ns_app = msg_no_arg(ns_app_cls, sel_shared_app);
        if !ns_app.is_null() {
            let sel_set_icon = sel_registerName(c"setApplicationIconImage:".as_ptr());
            let msg_set_icon: unsafe extern "C" fn(*mut c_void, *const c_void, *mut c_void) =
                std::mem::transmute(objc_msgSend as *const c_void);
            msg_set_icon(ns_app, sel_set_icon, ns_image);
        }

        let sel_release = sel_registerName(c"release".as_ptr());
        let msg_release: unsafe extern "C" fn(*mut c_void, *const c_void) =
            std::mem::transmute(objc_msgSend as *const c_void);
        msg_release(ns_image, sel_release);
    }
}

#[cfg(not(target_os = "macos"))]
pub fn set_app_icon(_icon_bytes: &[u8]) {}

#[cfg(target_os = "macos")]
pub fn show_about_panel() {
    use std::ffi::{c_char, c_void};

    #[link(name = "AppKit", kind = "framework")]
    unsafe extern "C" {
        static NSAboutPanelOptionApplicationName: *mut c_void;
        static NSAboutPanelOptionApplicationIcon: *mut c_void;
        static NSAboutPanelOptionApplicationVersion: *mut c_void;
        fn objc_getClass(name: *const c_char) -> *mut c_void;
        fn sel_registerName(name: *const c_char) -> *const c_void;
        fn objc_msgSend();
    }

    // The menu action runs on the app thread after GPUI initializes AppKit.
    unsafe {
        let msg_no_arg: unsafe extern "C" fn(*mut c_void, *const c_void) -> *mut c_void =
            std::mem::transmute(objc_msgSend as *const c_void);
        let app = msg_no_arg(
            objc_getClass(c"NSApplication".as_ptr()),
            sel_registerName(c"sharedApplication".as_ptr()),
        );
        let icon = msg_no_arg(app, sel_registerName(c"applicationIconImage".as_ptr()));
        let string_class = objc_getClass(c"NSString".as_ptr());
        let utf8_selector = sel_registerName(c"stringWithUTF8String:".as_ptr());
        let string_from_utf8: unsafe extern "C" fn(
            *mut c_void,
            *const c_void,
            *const c_char,
        ) -> *mut c_void = std::mem::transmute(objc_msgSend as *const c_void);
        let values = [
            string_from_utf8(string_class, utf8_selector, c"Block Clock".as_ptr()),
            icon,
            string_from_utf8(
                string_class,
                utf8_selector,
                concat!(env!("CARGO_PKG_VERSION"), "\0").as_ptr().cast(),
            ),
        ];
        let keys = [
            NSAboutPanelOptionApplicationName,
            NSAboutPanelOptionApplicationIcon,
            NSAboutPanelOptionApplicationVersion,
        ];
        let dictionary: unsafe extern "C" fn(
            *mut c_void,
            *const c_void,
            *const *mut c_void,
            *const *mut c_void,
            usize,
        ) -> *mut c_void = std::mem::transmute(objc_msgSend as *const c_void);
        let options = dictionary(
            objc_getClass(c"NSDictionary".as_ptr()),
            sel_registerName(c"dictionaryWithObjects:forKeys:count:".as_ptr()),
            values.as_ptr(),
            keys.as_ptr(),
            keys.len(),
        );

        let order_front: unsafe extern "C" fn(*mut c_void, *const c_void, *mut c_void) =
            std::mem::transmute(objc_msgSend as *const c_void);
        order_front(
            app,
            sel_registerName(c"orderFrontStandardAboutPanelWithOptions:".as_ptr()),
            options,
        );
    }
}
