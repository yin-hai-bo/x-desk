use std::{ffi::c_void, ptr::NonNull};

use windows::{
    Win32::{
        Foundation::{FALSE, GetLastError, HWND, LPARAM, SetLastError, WIN32_ERROR},
        Graphics::Gdi::UpdateWindow,
        UI::WindowsAndMessaging::{
            CREATESTRUCTW, CreateWindowExW, DestroyWindow, GWLP_USERDATA, GetWindowLongPtrW, GetWindowTextLengthW,
            GetWindowTextW, IsWindow, SHOW_WINDOW_CMD, SetWindowLongPtrW, ShowWindow,
        },
    },
    core::PCWSTR,
};

pub struct Window<T> {
    hwnd: HWND,
    component: T,
}

/// 将 Rust 对象与 HWND 关联起来，并可以利用 Rust 生命周期管理 Windows 窗口。
impl<T> Window<T> {
    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }

    pub fn component(&self) -> &T {
        &self.component
    }

    pub fn component_mut(&mut self) -> &mut T {
        &mut self.component
    }

    /// 调用 Win32 API 来创建在堆上的 Window<T> 对象
    pub fn create(
        exstyle: windows::Win32::UI::WindowsAndMessaging::WINDOW_EX_STYLE,
        class_name: PCWSTR,
        window_name: PCWSTR,
        style: windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        parent: Option<HWND>,
        menu: Option<windows::Win32::UI::WindowsAndMessaging::HMENU>,
        instance: Option<windows::Win32::Foundation::HINSTANCE>,
        component: T,
    ) -> anyhow::Result<Box<Self>> {
        let window = Box::new(Self {
            hwnd: HWND::default(),
            component,
        });
        let _ = unsafe {
            CreateWindowExW(
                exstyle,
                class_name,
                window_name,
                style,
                x,
                y,
                width,
                height,
                parent,
                menu,
                instance,
                Some(window.as_ref() as *const Self as *const c_void),
            )
        }?;
        Ok(window)
    }

    /// WndProc 处理 WM_NCCREATE 消息时调用本函数，将 Window 对象的指针与 HWND 关联起来
    pub fn on_wm_nccreate(hwnd: HWND, lparam: LPARAM) {
        unsafe {
            let cs = &*(lparam.0 as *const CREATESTRUCTW);
            let window = &mut *(cs.lpCreateParams as *mut Self);
            window.hwnd = hwnd;
            let _ = SetWindowLongPtrW(hwnd, GWLP_USERDATA, window as *mut Self as isize);
        }
    }

    /// WndProc 处理 WM_NCDESTROY 消息时调用本函数，将 Window 对象与 HWND 分离
    pub fn on_wm_ncdestroy(hwnd: HWND) {
        unsafe {
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut c_void;
            if ptr.is_null() {
                return;
            }
            let _ = SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            let window = &mut *(ptr as *mut Self);
            window.hwnd = HWND::default();
        }
    }

    /// 将本对象实例与 HWND 分离，不再关联。从此本对象不再负责 HWND 的销毁等。
    #[allow(dead_code)]
    pub fn detach_hwnd(&mut self) {
        if self.hwnd.is_invalid() {
            return;
        }
        unsafe {
            if FALSE != IsWindow(Some(self.hwnd)) {
                let _ = SetWindowLongPtrW(self.hwnd, GWLP_USERDATA, 0);
            }
        }
        self.hwnd = HWND::default();
    }

    pub fn get_self_from_hwnd(hwnd: HWND) -> Option<NonNull<Self>> {
        if unsafe { IsWindow(Some(hwnd)) } == FALSE {
            return None;
        }
        let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut c_void;
        if ptr.is_null() {
            return None;
        }
        NonNull::new(ptr as *mut Self)
    }

    pub fn show_window(&self, cmd: SHOW_WINDOW_CMD) -> bool {
        unsafe { ShowWindow(self.hwnd, cmd) != FALSE }
    }

    #[allow(dead_code)]
    pub fn update_window(&self) -> bool {
        unsafe { UpdateWindow(self.hwnd) != FALSE }
    }

    pub fn destroy_window(&mut self) -> anyhow::Result<()> {
        if self.hwnd.is_invalid() {
            anyhow::bail!("Cannot destroy an invalid window");
        }
        unsafe {
            if FALSE == IsWindow(Some(self.hwnd)) {
                self.hwnd = HWND::default();
                anyhow::bail!("Try to destroy a window, but IsWindow() return FALSE");
            }
            DestroyWindow(self.hwnd)?;
            self.hwnd = HWND::default();
        }
        Ok(())
    }

    pub fn is_window(&self) -> bool {
        if self.hwnd.is_invalid() {
            return false;
        }
        unsafe { IsWindow(Some(self.hwnd)) != FALSE }
    }

    pub fn get_window_text(&self) -> anyhow::Result<Vec<u16>> {
        if !self.is_window() {
            anyhow::bail!("Cannot get window text from an invalid window");
        }
        unsafe {
            SetLastError(WIN32_ERROR(0));
            let len = GetWindowTextLengthW(self.hwnd);
            if len <= 0 {
                let error_code = GetLastError();
                if error_code.is_err() {
                    anyhow::bail!("GetWindowTextLengthW() failed with error #{}", error_code.0);
                } else {
                    return Ok(vec![0]);
                }
            }

            let mut text = vec![0u16; (len + 1) as usize];
            SetLastError(WIN32_ERROR(0));
            let count = GetWindowTextW(self.hwnd, &mut text);
            if count == 0 {
                let error_code = GetLastError();
                if error_code.is_err() && len > 0 {
                    anyhow::bail!("GetWindowTextW() failed with error #{}", error_code.0);
                }
            }
            text.truncate(count as usize + 1);
            return Ok(text);
        }
    }
}

impl<T> Drop for Window<T> {
    fn drop(&mut self) {
        let _ = self.destroy_window();
        if !self.hwnd.is_invalid() {
            // 防止 Destroy Window 失败而没有处理 WM_NCDESTROY 消息
            // 所以这里手动将 HWND 中的关联裸指针清空
            unsafe {
                let _ = SetWindowLongPtrW(self.hwnd, GWLP_USERDATA, 0);
            }
        }
    }
}
