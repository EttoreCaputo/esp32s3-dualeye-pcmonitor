// No console window next to the app in Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use dualeye_core::claude::statusline;

fn main() {
    // Claude Code runs us as its status line command; handle that before the
    // single-instance check would hand it to the running app.
    if std::env::args().nth(1).as_deref() == Some(statusline::FLAG) {
        statusline::run(std::io::stdin().lock(), std::io::stdout().lock());
        return;
    }
    dualeye_app_lib::run();
}
