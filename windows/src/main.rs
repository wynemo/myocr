#![windows_subsystem = "windows"]

mod ocr;

use std::{
    ffi::c_void,
    mem::{size_of, zeroed},
    ptr::{null, null_mut},
    sync::mpsc,
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::{
        Com::*, DataExchange::*, Diagnostics::Debug::MessageBeep, LibraryLoader::GetModuleHandleW,
        Memory::*, Threading::*,
    },
    UI::{HiDpi::*, Input::KeyboardAndMouse::*, Shell::*, WindowsAndMessaging::*},
};

const TRAY: u32 = WM_APP + 1;
const COMPLETE: u32 = WM_APP + 2;
const CAPTURE: u16 = 1;
const QUIT: u16 = 2;
const CHINESE_SETUP: u16 = 3;
const AUTO: u16 = 100;
const LANGUAGE: u16 = 101;

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}

unsafe fn dialog(hwnd: HWND, text: &str) {
    MessageBoxW(
        hwnd,
        wide(text).as_ptr(),
        wide("MyOCR").as_ptr(),
        MB_OK | MB_ICONINFORMATION,
    );
}

struct Screenshot {
    dc: HDC,
    bitmap: HBITMAP,
    old: HGDIOBJ,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

impl Screenshot {
    unsafe fn capture() -> Result<Self, String> {
        let x = GetSystemMetrics(SM_XVIRTUALSCREEN);
        let y = GetSystemMetrics(SM_YVIRTUALSCREEN);
        let width = GetSystemMetrics(SM_CXVIRTUALSCREEN);
        let height = GetSystemMetrics(SM_CYVIRTUALSCREEN);
        let screen = GetDC(null_mut());
        if screen.is_null() {
            return Err("Cannot access the screen.".into());
        }
        let dc = CreateCompatibleDC(screen);
        let bitmap = CreateCompatibleBitmap(screen, width, height);
        if dc.is_null() || bitmap.is_null() {
            if !dc.is_null() {
                DeleteDC(dc);
            }
            if !bitmap.is_null() {
                DeleteObject(bitmap);
            }
            ReleaseDC(null_mut(), screen);
            return Err("Cannot allocate the screen capture.".into());
        }
        let old = SelectObject(dc, bitmap);
        let ok = BitBlt(dc, 0, 0, width, height, screen, x, y, SRCCOPY | CAPTUREBLT);
        ReleaseDC(null_mut(), screen);
        let shot = Self {
            dc,
            bitmap,
            old,
            x,
            y,
            width,
            height,
        };
        if ok == 0 {
            return Err("Screen capture failed.".into());
        }
        Ok(shot)
    }

    unsafe fn crop(&self, rect: RECT) -> Result<(Vec<u8>, i32, i32), String> {
        let width = rect.right - rect.left;
        let height = rect.bottom - rect.top;
        let dc = CreateCompatibleDC(self.dc);
        if dc.is_null() {
            return Err("Cannot create capture DC.".into());
        }
        let mut info: BITMAPINFO = zeroed();
        info.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
        info.bmiHeader.biWidth = width;
        info.bmiHeader.biHeight = -height;
        info.bmiHeader.biPlanes = 1;
        info.bmiHeader.biBitCount = 32;
        info.bmiHeader.biCompression = BI_RGB;
        let mut bits: *mut c_void = null_mut();
        let bitmap = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
        if bitmap.is_null() {
            DeleteDC(dc);
            return Err("Cannot allocate selection bitmap.".into());
        }
        let old = SelectObject(dc, bitmap);
        let ok = BitBlt(
            dc, 0, 0, width, height, self.dc, rect.left, rect.top, SRCCOPY,
        );
        GdiFlush();
        let pixels = if ok != 0 {
            Some(
                std::slice::from_raw_parts(bits as *const u8, width as usize * height as usize * 4)
                    .to_vec(),
            )
        } else {
            None
        };
        SelectObject(dc, old);
        DeleteObject(bitmap);
        DeleteDC(dc);
        pixels
            .map(|p| (p, width, height))
            .ok_or_else(|| "Cannot copy the selection.".into())
    }
}

impl Drop for Screenshot {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.dc, self.old);
            DeleteObject(self.bitmap);
            DeleteDC(self.dc);
        }
    }
}

struct App {
    overlay: HWND,
    screenshot: Option<Screenshot>,
    start: Option<POINT>,
    end: POINT,
    dragging: bool,
    busy: bool,
    languages: Vec<(String, String)>,
    selected: Option<usize>,
    sender: mpsc::Sender<Result<String, String>>,
    receiver: mpsc::Receiver<Result<String, String>>,
}

unsafe fn app_ptr(hwnd: HWND) -> *mut App {
    GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut App
}

fn selection(start: POINT, end: POINT, width: i32, height: i32) -> RECT {
    RECT {
        left: start.x.min(end.x).clamp(0, width),
        top: start.y.min(end.y).clamp(0, height),
        right: start.x.max(end.x).clamp(0, width),
        bottom: start.y.max(end.y).clamp(0, height),
    }
}

fn mouse_point(lparam: LPARAM) -> POINT {
    POINT {
        x: lparam as u16 as i16 as i32,
        y: (lparam >> 16) as u16 as i16 as i32,
    }
}

unsafe fn notify(hwnd: HWND, message: &str, error: bool) {
    let mut data: NOTIFYICONDATAW = zeroed();
    data.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
    data.hWnd = hwnd;
    data.uID = 1;
    data.uFlags = NIF_INFO;
    data.dwInfoFlags = if error { NIIF_ERROR } else { NIIF_INFO };
    let title = wide("MyOCR");
    data.szInfoTitle[..title.len()].copy_from_slice(&title);
    let text: Vec<u16> = message.encode_utf16().take(254).collect();
    data.szInfo[..text.len()].copy_from_slice(&text);
    Shell_NotifyIconW(NIM_MODIFY, &data);
}

unsafe fn copy_text(hwnd: HWND, text: &str) -> Result<(), String> {
    // Clipboard can be briefly held by another application.
    let mut opened = false;
    for _ in 0..10 {
        if OpenClipboard(hwnd) != 0 {
            opened = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    if !opened {
        return Err("Clipboard is busy. Try again.".into());
    }
    let encoded = wide(text);
    let memory = GlobalAlloc(GMEM_MOVEABLE, encoded.len() * 2);
    if memory.is_null() {
        CloseClipboard();
        return Err("Cannot allocate clipboard text.".into());
    }
    let pointer = GlobalLock(memory) as *mut u16;
    if pointer.is_null() {
        GlobalFree(memory);
        CloseClipboard();
        return Err("Cannot write clipboard text.".into());
    }
    std::ptr::copy_nonoverlapping(encoded.as_ptr(), pointer, encoded.len());
    GlobalUnlock(memory);
    let ok = EmptyClipboard() != 0 && !SetClipboardData(13, memory).is_null();
    if !ok {
        GlobalFree(memory);
    }
    CloseClipboard();
    if ok {
        Ok(())
    } else {
        Err("Cannot update the clipboard.".into())
    }
}

unsafe fn cancel(pointer: *mut App) {
    // ReleaseCapture/ShowWindow can synchronously re-enter the window procedure.
    // Finish state changes and end the borrow before calling either API.
    let overlay = {
        let state = &mut *pointer;
        state.dragging = false;
        state.screenshot = None;
        state.start = None;
        state.overlay
    };
    ReleaseCapture();
    ShowWindow(overlay, SW_HIDE);
}

unsafe fn begin(hwnd: HWND) {
    let state = &mut *app_ptr(hwnd);
    if state.busy || state.screenshot.is_some() {
        return;
    }
    match Screenshot::capture() {
        Ok(shot) => {
            let (x, y, width, height) = (shot.x, shot.y, shot.width, shot.height);
            state.screenshot = Some(shot);
            state.start = None;
            state.dragging = false;
            let overlay = state.overlay;
            SetWindowPos(overlay, HWND_TOPMOST, x, y, width, height, SWP_SHOWWINDOW);
            SetForegroundWindow(overlay);
            SetFocus(overlay);
            InvalidateRect(overlay, null(), 0);
        }
        Err(error) => notify(hwnd, &error, true),
    }
}

unsafe fn menu(hwnd: HWND) {
    let state = &mut *app_ptr(hwnd);
    let menu = CreatePopupMenu();
    AppendMenuW(
        menu,
        MF_STRING | if state.busy { MF_GRAYED } else { 0 },
        CAPTURE as usize,
        wide(if state.busy {
            "Recognizing..."
        } else {
            "Capture text (Ctrl+Alt+O)"
        })
        .as_ptr(),
    );
    let languages = CreatePopupMenu();
    AppendMenuW(
        languages,
        MF_STRING
            | if state.selected.is_none() {
                MF_CHECKED
            } else {
                0
            },
        AUTO as usize,
        wide("Use Windows profile language").as_ptr(),
    );
    for (i, (tag, name)) in state.languages.iter().enumerate() {
        AppendMenuW(
            languages,
            MF_STRING
                | if state.selected == Some(i) {
                    MF_CHECKED
                } else {
                    0
                },
            LANGUAGE as usize + i,
            wide(&format!("{name} ({tag})")).as_ptr(),
        );
    }
    AppendMenuW(
        menu,
        MF_POPUP,
        languages as usize,
        wide("OCR language").as_ptr(),
    );
    AppendMenuW(
        menu,
        MF_STRING,
        CHINESE_SETUP as usize,
        wide("Chinese OCR setup...").as_ptr(),
    );
    AppendMenuW(menu, MF_SEPARATOR, 0, null());
    AppendMenuW(menu, MF_STRING, QUIT as usize, wide("Quit MyOCR").as_ptr());
    let mut point = zeroed();
    GetCursorPos(&mut point);
    SetForegroundWindow(hwnd);
    let command = TrackPopupMenu(
        menu,
        TPM_RETURNCMD | TPM_RIGHTBUTTON,
        point.x,
        point.y,
        0,
        hwnd,
        null(),
    );
    DestroyMenu(menu);
    PostMessageW(hwnd, WM_NULL, 0, 0);
    match command as u16 {
        CAPTURE => begin(hwnd),
        QUIT => {
            DestroyWindow(hwnd);
        }
        CHINESE_SETUP => dialog(hwnd,
            "中文识别：在 OCR language 菜单选择中文（zh-Hans / zh-Hant）。\n\n如果没有中文，请在管理员 PowerShell 中安装系统简体中文 OCR 组件：\n\nAdd-WindowsCapability -Online -Name Language.OCR~~~zh-CN~0.0.1.0\n\n安装完成后退出并重启 MyOCR。中文输入法不等于中文 OCR 组件。\n\nChinese OCR requires an installed Windows OCR language pack."),
        AUTO => (*app_ptr(hwnd)).selected = None,
        id if id >= LANGUAGE && (id - LANGUAGE) < (*app_ptr(hwnd)).languages.len() as u16 => {
            (*app_ptr(hwnd)).selected = Some((id - LANGUAGE) as usize);
        }
        _ => {}
    }
}

unsafe extern "system" fn overlay_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let pointer = app_ptr(hwnd);
    if pointer.is_null() {
        return DefWindowProcW(hwnd, message, wparam, lparam);
    }
    let state = &mut *pointer;
    match message {
        WM_ERASEBKGND => 1,
        WM_PAINT => {
            let mut paint = zeroed();
            let dc = BeginPaint(hwnd, &mut paint);
            if let Some(shot) = &state.screenshot {
                BitBlt(dc, 0, 0, shot.width, shot.height, shot.dc, 0, 0, SRCCOPY);
                SetBkMode(dc, TRANSPARENT as i32);
                SetTextColor(dc, 0x00ffffff);
                let text = wide("Drag to select text. Esc / right-click to cancel.");
                let mut banner = RECT {
                    left: 0,
                    top: 0,
                    right: shot.width,
                    bottom: 42,
                };
                FillRect(dc, &banner, GetStockObject(BLACK_BRUSH) as HBRUSH);
                banner.left = 14;
                banner.top = 10;
                DrawTextW(dc, text.as_ptr(), -1, &mut banner, DT_LEFT | DT_SINGLELINE);
                if let Some(start) = state.start {
                    let rect = selection(start, state.end, shot.width, shot.height);
                    DrawFocusRect(dc, &rect);
                }
            }
            EndPaint(hwnd, &paint);
            0
        }
        WM_LBUTTONDOWN => {
            state.start = Some(mouse_point(lparam));
            state.end = mouse_point(lparam);
            state.dragging = true;
            SetCapture(hwnd);
            0
        }
        WM_MOUSEMOVE if state.dragging => {
            state.end = mouse_point(lparam);
            InvalidateRect(hwnd, null(), 0);
            0
        }
        WM_LBUTTONUP if state.dragging => {
            state.dragging = false;
            state.end = mouse_point(lparam);
            let crop = state.screenshot.as_ref().and_then(|shot| {
                let rect = selection(state.start?, state.end, shot.width, shot.height);
                (rect.right - rect.left >= 3 && rect.bottom - rect.top >= 3)
                    .then(|| shot.crop(rect))
            });
            cancel(pointer);
            if let Some(crop) = crop {
                let state = &mut *pointer;
                let owner = GetWindow(hwnd, GW_OWNER);
                match crop {
                    Ok((pixels, width, height)) => {
                        state.busy = true;
                        let tag = state.selected.map(|i| state.languages[i].0.clone());
                        let sender = state.sender.clone();
                        let owner_id = owner as usize;
                        std::thread::spawn(move || {
                            let status = CoInitializeEx(null(), COINIT_MULTITHREADED as u32);
                            let result = if status < 0 {
                                Err(format!("Cannot initialize OCR (0x{:08X}).", status as u32))
                            } else {
                                let result = ocr::recognize(&pixels, width, height, tag.as_deref())
                                    .map_err(|e| format!("OCR failed: {e}\nCheck installed Windows OCR language packs."));
                                CoUninitialize();
                                result
                            };
                            let _ = sender.send(result);
                            PostMessageW(owner_id as HWND, COMPLETE, 0, 0);
                        });
                    }
                    Err(error) => notify(owner, &error, true),
                }
            }
            0
        }
        WM_KEYDOWN if wparam == VK_ESCAPE as usize => {
            cancel(pointer);
            0
        }
        WM_RBUTTONDOWN | WM_CLOSE => {
            cancel(pointer);
            0
        }
        WM_ACTIVATE if (wparam & 0xffff) == WA_INACTIVE as usize => {
            if state.screenshot.is_some() {
                cancel(pointer);
            }
            0
        }
        WM_CAPTURECHANGED if state.dragging => {
            cancel(pointer);
            0
        }
        _ => DefWindowProcW(hwnd, message, wparam, lparam),
    }
}

unsafe extern "system" fn owner_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_HOTKEY => {
            begin(hwnd);
            0
        }
        TRAY => {
            if lparam as u32 == WM_RBUTTONUP {
                menu(hwnd);
            }
            if lparam as u32 == WM_LBUTTONUP {
                begin(hwnd);
            }
            0
        }
        COMPLETE => {
            let state = &mut *app_ptr(hwnd);
            state.busy = false;
            if let Ok(result) = state.receiver.try_recv() {
                match result {
                    Ok(text) if text.is_empty() => {
                        notify(hwnd, "No text found. Clipboard unchanged.", false)
                    }
                    Ok(text) => match copy_text(hwnd, &text) {
                        Ok(()) => {
                            MessageBeep(MB_OK);
                            notify(hwnd, "Text copied to clipboard.", false);
                        }
                        Err(error) => notify(hwnd, &error, true),
                    },
                    Err(error) => {
                        notify(hwnd, &error, true);
                        dialog(hwnd, &error);
                    }
                }
            }
            0
        }
        WM_DESTROY => {
            UnregisterHotKey(hwnd, 1);
            let mut data: NOTIFYICONDATAW = zeroed();
            data.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
            data.hWnd = hwnd;
            data.uID = 1;
            Shell_NotifyIconW(NIM_DELETE, &data);
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, message, wparam, lparam),
    }
}

unsafe fn run() -> Result<(), String> {
    SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    let mutex = CreateMutexW(null(), 0, wide("Local\\wynemo.MyOCR").as_ptr());
    if mutex.is_null() {
        return Err("Cannot create the application lock.".into());
    }
    if GetLastError() == ERROR_ALREADY_EXISTS {
        CloseHandle(mutex);
        return Err("MyOCR is already running. Look in the system tray.".into());
    }
    let result = run_app();
    CloseHandle(mutex);
    result
}

unsafe fn run_app() -> Result<(), String> {
    let languages = ocr::languages().map_err(|e| format!("Cannot initialize Windows OCR: {e}"))?;
    if languages.is_empty() {
        return Err("No Windows OCR languages installed. Open Settings > Time & language > Language & region > Language options and install an OCR-capable language (for example English or Chinese), then restart MyOCR.".into());
    }
    // The profile API selects one language, which may be English even when
    // Chinese OCR is available. Prefer an installed Chinese engine on startup.
    let selected = languages.iter().position(|(tag, _)| {
        tag.split('-')
            .next()
            .is_some_and(|base| base.eq_ignore_ascii_case("zh"))
    });
    let (sender, receiver) = mpsc::channel();
    let mut state = Box::new(App {
        overlay: null_mut(),
        screenshot: None,
        start: None,
        end: POINT { x: 0, y: 0 },
        dragging: false,
        busy: false,
        languages,
        selected,
        sender,
        receiver,
    });
    let instance = GetModuleHandleW(null());
    let class = wide("MyOCR.Owner");
    let overlay_class = wide("MyOCR.Selection");
    let mut wc: WNDCLASSW = zeroed();
    wc.hInstance = instance;
    wc.lpszClassName = class.as_ptr();
    wc.lpfnWndProc = Some(owner_proc);
    if RegisterClassW(&wc) == 0 {
        return Err("Cannot register application window.".into());
    }
    wc.lpszClassName = overlay_class.as_ptr();
    wc.lpfnWndProc = Some(overlay_proc);
    wc.hCursor = LoadCursorW(null_mut(), IDC_CROSS);
    if RegisterClassW(&wc) == 0 {
        return Err("Cannot register selection window.".into());
    }
    let owner = CreateWindowExW(
        WS_EX_TOOLWINDOW,
        class.as_ptr(),
        wide("MyOCR").as_ptr(),
        WS_POPUP,
        0,
        0,
        0,
        0,
        null_mut(),
        null_mut(),
        instance,
        null(),
    );
    if owner.is_null() {
        return Err("Cannot create application window.".into());
    }
    SetWindowLongPtrW(owner, GWLP_USERDATA, &mut *state as *mut App as isize);
    let overlay = CreateWindowExW(
        WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
        overlay_class.as_ptr(),
        wide("MyOCR selection").as_ptr(),
        WS_POPUP,
        0,
        0,
        0,
        0,
        owner,
        null_mut(),
        instance,
        null(),
    );
    if overlay.is_null() {
        DestroyWindow(owner);
        return Err("Cannot create selection window.".into());
    }
    state.overlay = overlay;
    SetWindowLongPtrW(overlay, GWLP_USERDATA, &mut *state as *mut App as isize);
    if RegisterHotKey(owner, 1, MOD_CONTROL | MOD_ALT | MOD_NOREPEAT, b'O' as u32) == 0 {
        DestroyWindow(overlay);
        DestroyWindow(owner);
        return Err(
            "Ctrl+Alt+O is already used by another application. Close it and restart MyOCR.".into(),
        );
    }
    let mut data: NOTIFYICONDATAW = zeroed();
    data.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
    data.hWnd = owner;
    data.uID = 1;
    data.uFlags = NIF_ICON | NIF_MESSAGE | NIF_TIP;
    data.uCallbackMessage = TRAY;
    data.hIcon = LoadIconW(null_mut(), IDI_APPLICATION);
    let tip = wide("MyOCR - Ctrl+Alt+O / right-click for menu");
    data.szTip[..tip.len()].copy_from_slice(&tip);
    if Shell_NotifyIconW(NIM_ADD, &data) == 0 {
        DestroyWindow(overlay);
        DestroyWindow(owner);
        return Err("Cannot add the system tray icon.".into());
    }
    let language = state
        .selected
        .map(|i| state.languages[i].0.as_str())
        .unwrap_or("Windows profile; Chinese OCR not available");
    notify(owner, &format!("Ready. OCR: {language}. Press Ctrl+Alt+O. Right-click the tray icon for OCR language / Chinese OCR setup."), false);
    let mut message = zeroed();
    loop {
        let status = GetMessageW(&mut message, null_mut(), 0, 0);
        if status <= 0 {
            break;
        }
        TranslateMessage(&message);
        DispatchMessageW(&message);
    }
    // Owned windows are destroyed with their owner, before App is dropped.
    Ok(())
}

fn main() {
    unsafe {
        let status = CoInitializeEx(null(), COINIT_APARTMENTTHREADED as u32);
        if status < 0 {
            dialog(null_mut(), "Cannot initialize Windows Runtime.");
            return;
        }
        if let Err(error) = run() {
            dialog(null_mut(), &error);
        }
        CoUninitialize();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_normalizes_reverse_drag_and_clips_to_screen() {
        let r = selection(POINT { x: 800, y: 600 }, POINT { x: -20, y: 100 }, 640, 480);
        assert_eq!((r.left, r.top, r.right, r.bottom), (0, 100, 640, 480));
    }

    #[test]
    fn mouse_coordinates_are_signed() {
        let p = mouse_point(((25u32 << 16) | (-10i16 as u16 as u32)) as isize);
        assert_eq!((p.x, p.y), (-10, 25));
    }
}
