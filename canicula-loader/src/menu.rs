use core::fmt::Write;

use uefi::proto::console::text::{Color, Key, ScanCode};

use crate::config::{BOOT_ENTRIES, BOOT_TIMEOUT_SECS, BootMode, DEFAULT_ENTRY};

/// Display the boot menu and return the selected boot mode.
///
/// Shows a TUI with selectable boot entries, arrow key navigation,
/// and an auto-boot countdown timer. Any key press cancels the timer.
pub fn show_boot_menu() -> BootMode {
    let mut selected = DEFAULT_ENTRY;
    let mut timeout: Option<usize> = Some(BOOT_TIMEOUT_SECS);
    let mut tick_count: usize = 0;

    // Clear screen and hide cursor
    uefi::system::with_stdout(|out| {
        let _ = out.clear();
        let _ = out.enable_cursor(false);
    });

    draw_menu(selected, timeout);

    loop {
        // Sleep 100ms per tick
        uefi::boot::stall(100_000);

        // Non-blocking key read
        let key = uefi::system::with_stdin(|stdin| stdin.read_key());

        if let Ok(Some(key)) = key {
            // Any key press cancels the auto-boot timer
            timeout = None;

            match key {
                Key::Special(ScanCode::UP) => {
                    if selected > 0 {
                        selected -= 1;
                    }
                }
                Key::Special(ScanCode::DOWN) => {
                    if selected < BOOT_ENTRIES.len() - 1 {
                        selected += 1;
                    }
                }
                Key::Printable(c) if u16::from(c) == 0x000D => {
                    // Enter key (carriage return)
                    boot_selected(selected);
                    return BOOT_ENTRIES[selected].mode;
                }
                _ => {}
            }

            draw_menu(selected, timeout);
        }

        // Countdown: 10 ticks = ~1 second
        tick_count += 1;
        if tick_count >= 10 {
            tick_count = 0;
            if let Some(ref mut t) = timeout {
                if *t == 0 {
                    boot_selected(selected);
                    return BOOT_ENTRIES[selected].mode;
                }
                *t -= 1;
                draw_menu(selected, timeout);
            }
        }
    }
}

/// Clear screen and show a boot message before handing off
fn boot_selected(selected: usize) {
    uefi::system::with_stdout(|out| {
        let _ = out.set_color(Color::White, Color::Black);
        let _ = out.clear();
        let _ = write!(out, "Booting {}...\n", BOOT_ENTRIES[selected].name);
    });
}

fn draw_menu(selected: usize, timeout: Option<usize>) {
    uefi::system::with_stdout(|out| {
        let _ = out.set_cursor_position(0, 0);

        // Title
        let _ = out.set_color(Color::White, Color::Black);
        let _ = write!(out, "\n");
        let _ = write!(out, "  Canicula Boot Loader\n");
        let _ = write!(out, "\n");

        // Boot entries
        for (i, entry) in BOOT_ENTRIES.iter().enumerate() {
            if i == selected {
                let _ = out.set_color(Color::White, Color::Blue);
                let _ = write!(out, "  {:<70}\n", entry.name);
                let _ = out.set_color(Color::White, Color::Black);
            } else {
                let _ = out.set_color(Color::LightGray, Color::Black);
                let _ = write!(out, "  {:<70}\n", entry.name);
            }
        }

        // Spacing
        let _ = out.set_color(Color::LightGray, Color::Black);
        let _ = write!(out, "\n");

        // Timeout line
        match timeout {
            Some(secs) => {
                let _ = write!(
                    out,
                    "  Boot in {}s...                              \n",
                    secs
                );
            }
            None => {
                let _ = write!(out, "                                              \n");
            }
        }

        // Help text
        let _ = out.set_color(Color::DarkGray, Color::Black);
        let _ = write!(out, "  Up/Down to select, Enter to boot\n");
        let _ = out.set_color(Color::White, Color::Black);
    });
}
