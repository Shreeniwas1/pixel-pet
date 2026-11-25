//! Pixel Pet: a tiny always-on-top desktop pet with a smart todo list, focus timer and stats.
//! Plain Win32 + GDI, no runtime, no webview.
#![windows_subsystem = "windows"]

mod anim;
mod art;
mod autostart;
mod ball;
mod body;
mod buddy;
mod calc;
mod care;
mod cast;
mod food;
mod gfx;
mod hub;
mod instance;
mod intro;
mod keys;
mod launch;
mod media;
mod mood;
mod music;
mod opencode;
mod paint;
mod perch;
mod petwnd;
mod place;
mod prelude;
mod presence;
mod rig;
mod rules;
mod sound;
mod state;
mod stats;
mod statswin;
mod store;
mod sys;
mod talk;
mod tick;
mod todo;
mod tokens;
mod todowin;
mod tools;
mod tray;
mod wardrobe;
mod world;

use prelude::*;
use windows_sys::Win32::UI::HiDpi::{SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2};

fn main() {
    unsafe {
        if !instance::may_start() {
            return; // there already is a pet, and it has been told
        }
        // real pixels on every monitor, so the pet knows where they touch
        if SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) == 0 {
            SetProcessDPIAware();
        }
        let screen = GetDC(null_mut());
        let scale = GetDeviceCaps(screen, LOGPIXELSX as i32) as f32 / 96.0;
        ReleaseDC(null_mut(), screen);
        {
            let mut s = ST.lock().unwrap();
            s.scale = scale;
            load_cfg(&mut s);
            if std::env::var_os("PIXEL_PET_TEST").is_some_and(|v| v != "busy") {
                s.dnd_on = false; // integration tests must not depend on what is on screen
                s.intro = intro::DONE;
            }
            set_theme(s.theme);
            sound::ON.store(s.sound_on, Relaxed);
            load_usage(&mut s);
            s.launch = launch::load();
            tokens::spawn();
            music::spawn();
            buddy::spawn();
            media::spawn();
            s.shortcut = instance::has_shortcut();
            if s.shortcut {
                instance::refresh();
            }
            s.autostart = autostart::is_enabled();
            if s.autostart && std::env::var_os("PIXEL_PET_TEST").is_none() {
                autostart::refresh(); // keep the entry pointing at this exe if it moved
            }
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
        tools::register(hinst);
        ball::register(hinst);

        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            class.as_ptr(),
            wide("Pixel Pet").as_ptr(),
            WS_POPUP,
            0, 0, 10, 10,
            null_mut(), null_mut(), hinst, null(),
        );
        PET_WND.store(hwnd as isize, Relaxed);
        instance::claim(hwnd);
        windows_sys::Win32::UI::Shell::DragAcceptFiles(hwnd, 1);
        SetLayeredWindowAttributes(hwnd, KEY, 0, LWA_COLORKEY);
        place(hwnd);
        presence::sync(hwnd);
        tray::add(hwnd);
        {
            let s = ST.lock().unwrap();
            keys::register(hwnd, &s.keys, s.shortcut);
        }

        let mut msg: MSG = zeroed();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            if tools::key(&msg) {
                continue;
            }
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
