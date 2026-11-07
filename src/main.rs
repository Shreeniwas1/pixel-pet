//! Pixel Pet: a tiny always-on-top desktop pet with a smart todo list, focus timer and stats.
//! Plain Win32 + GDI, no runtime, no webview.
#![windows_subsystem = "windows"]

mod art;
mod care;
mod gfx;
mod hub;
mod intro;
mod mood;
mod paint;
mod petwnd;
mod place;
mod prelude;
mod rules;
mod state;
mod stats;
mod statswin;
mod store;
mod stroll;
mod sys;
mod talk;
mod tick;
mod todo;
mod tokens;
mod todowin;

use prelude::*;

fn main() {
    unsafe {
        SetProcessDPIAware();
        let screen = GetDC(null_mut());
        let scale = GetDeviceCaps(screen, LOGPIXELSX as i32) as f32 / 96.0;
        ReleaseDC(null_mut(), screen);
        {
            let mut s = ST.lock().unwrap();
            s.scale = scale;
            load_cfg(&mut s);
            if std::env::var_os("PIXEL_PET_TEST").is_some() {
                s.dnd_on = false; // integration tests must not depend on what is on screen
                s.intro = intro::DONE;
            }
            load_usage(&mut s);
            tokens::spawn();
            if let Some(t) = data_path("pixel-pet-focus.txt").and_then(|p| std::fs::read_to_string(p).ok()) {
                s.stats.load_text(&t);
            }
            if let Some(p) = data_path("pixel-pet-todos.txt") {
                s.todos = todo::load(&p);
                todo::sort(&mut s.todos);
                s.next_id = s.todos.len() as u32 + 1;
            }
            (s.last_in, s.last_out) = totals();
            let (now, ..) = local_now();
            if let Some(ev) = s.mood.on_launch(now) {
                say(&mut s, ev);
            }
            s.fact = (std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.subsec_nanos() as usize)
                .unwrap_or(0))
                % fact_count();
        }

        let hinst = GetModuleHandleW(null());
        let class = wide("PixelPetWnd");
        let wc = WNDCLASSW {
            style: 0,
            lpfnWndProc: Some(wndproc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinst,
            hIcon: null_mut(),
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            hbrBackground: null_mut(),
            lpszMenuName: null(),
            lpszClassName: class.as_ptr(),
        };
        RegisterClassW(&wc);
        let todo_class = wide("PixelTodoWnd");
        let tc = WNDCLASSW {
            style: 0,
            lpfnWndProc: Some(todo_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinst,
            hIcon: null_mut(),
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            hbrBackground: (COLOR_BTNFACE + 1) as usize as HBRUSH,
            lpszMenuName: null(),
            lpszClassName: todo_class.as_ptr(),
        };
        RegisterClassW(&tc);
        let stats_class = wide("PixelStatsWnd");
        let sc = WNDCLASSW {
            style: 0,
            lpfnWndProc: Some(stats_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinst,
            hIcon: null_mut(),
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            hbrBackground: null_mut(),
            lpszMenuName: null(),
            lpszClassName: stats_class.as_ptr(),
        };
        RegisterClassW(&sc);

        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            class.as_ptr(),
            wide("Pixel Pet").as_ptr(),
            WS_POPUP,
            0, 0, 10, 10,
            null_mut(), null_mut(), hinst, null(),
        );
        PET_WND.store(hwnd as isize, Relaxed);
        SetLayeredWindowAttributes(hwnd, KEY, 0, LWA_COLORKEY);
        place(hwnd);
        ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        // Ctrl+Alt+T: quick-add a task, Ctrl+Alt+F: toggle focus mode, Ctrl+Alt+S: stats (skipped if taken)
        RegisterHotKey(hwnd, 1, MOD_CONTROL | MOD_ALT | MOD_NOREPEAT, 'T' as u32);
        RegisterHotKey(hwnd, 2, MOD_CONTROL | MOD_ALT | MOD_NOREPEAT, 'F' as u32);
        RegisterHotKey(hwnd, 3, MOD_CONTROL | MOD_ALT | MOD_NOREPEAT, 'S' as u32); // stats window

        let mut msg: MSG = zeroed();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            // Enter in the todo box adds the item
            if msg.message == WM_KEYDOWN && msg.wParam == VK_RETURN as usize && msg.hwnd == hw(&TODO_EDIT) {
                add_from_edit();
                continue;
            }
            if msg.message == WM_KEYDOWN && msg.wParam == VK_ESCAPE as usize {
                let st = hw(&STATS_WND);
                if !st.is_null() && IsWindowVisible(st) != 0 && msg.hwnd == st {
                    ShowWindow(st, SW_HIDE);
                    continue;
                }
                let t = hw(&TODO_WND);
                if !t.is_null() && IsWindowVisible(t) != 0 && (msg.hwnd == t || msg.hwnd == hw(&TODO_EDIT)) {
                    ShowWindow(t, SW_HIDE);
                    continue;
                }
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}
