use eframe::egui::{Rect, Vec2};
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, GWL_STYLE, GetWindowLongPtrW, MoveWindow, SW_HIDE,
    SW_SHOWNOACTIVATE, SetWindowLongPtrW, ShowWindow, WS_CHILD, WS_CLIPCHILDREN, WS_DISABLED,
};

pub struct VideoSurface {
    hwnd: HWND,
    bounds: [i32; 4],
    visible: bool,
}

impl VideoSurface {
    pub fn new(parent: isize) -> Result<Self, String> {
        let class: Vec<u16> = "STATIC\0".encode_utf16().collect();
        // The system STATIC class provides a child HWND for mpv's video output.
        let hwnd = unsafe {
            let parent = parent as HWND;
            let style = GetWindowLongPtrW(parent, GWL_STYLE);
            SetWindowLongPtrW(parent, GWL_STYLE, style | WS_CLIPCHILDREN as isize);
            CreateWindowExW(
                0,
                class.as_ptr(),
                std::ptr::null(),
                WS_CHILD | WS_CLIPCHILDREN | WS_DISABLED,
                0,
                0,
                1,
                1,
                parent,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null(),
            )
        };
        if hwnd.is_null() {
            return Err(std::io::Error::last_os_error().to_string());
        }
        Ok(Self {
            hwnd,
            bounds: [0; 4],
            visible: false,
        })
    }

    pub fn hide(&mut self) {
        if self.visible {
            unsafe {
                ShowWindow(self.hwnd, SW_HIDE);
            }
            self.visible = false;
        }
    }

    pub fn handle(&self) -> isize {
        self.hwnd as isize
    }

    pub fn place(&mut self, rect: Rect, origin: Vec2, scale: f32, visible: bool) {
        let rect = rect.translate(-origin);
        let bounds = [rect.min.x, rect.min.y, rect.width(), rect.height()]
            .map(|x| (x * scale).round() as i32);
        if self.bounds != bounds {
            self.bounds = bounds;
            unsafe {
                MoveWindow(
                    self.hwnd,
                    bounds[0],
                    bounds[1],
                    bounds[2].max(1),
                    bounds[3].max(1),
                    1,
                );
            }
        }
        if self.visible != visible {
            self.visible = visible;
            unsafe {
                ShowWindow(self.hwnd, if visible { SW_SHOWNOACTIVATE } else { SW_HIDE });
            }
        }
    }
}

impl Drop for VideoSurface {
    fn drop(&mut self) {
        unsafe {
            DestroyWindow(self.hwnd);
        }
    }
}
