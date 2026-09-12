//! Minimal terminal control: raw mode, size, writes, and reply reads.
//!
//! Everything terminal-specific lives here so `kitty.rs` and `scene.rs` stay
//! testable without a terminal.

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::io::{AsRawFd, RawFd};
use std::sync::mpsc::{self, Receiver};
use std::thread;

/// Terminal cell grid and pixel size.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Size {
    pub cols: u16,
    pub rows: u16,
    pub xpixel: u16,
    pub ypixel: u16,
}

/// Owning handle for the controlling terminal.
pub struct Terminal {
    file: File,
    pub is_tty: bool,
    raw: bool,
    saved: Option<libc::termios>,
}

impl Terminal {
    /// Open `/dev/tty`, falling back to stdout when there is no controlling
    /// terminal (for example under a pipe).
    pub fn open() -> io::Result<Self> {
        match OpenOptions::new().read(true).write(true).open("/dev/tty") {
            Ok(file) => {
                let is_tty = unsafe { libc::isatty(file.as_raw_fd()) } == 1;
                Ok(Self {
                    file,
                    is_tty,
                    raw: false,
                    saved: None,
                })
            }
            Err(_) => Ok(Self {
                file: OpenOptions::new().write(true).open("/dev/stdout")?,
                is_tty: false,
                raw: false,
                saved: None,
            }),
        }
    }

    pub fn fd(&self) -> RawFd {
        self.file.as_raw_fd()
    }

    /// Put the terminal in raw mode, remembering the previous settings.
    pub fn enter_raw(&mut self) -> io::Result<()> {
        if !self.is_tty || self.raw {
            return Ok(());
        }
        let fd = self.fd();
        let mut termios: libc::termios = unsafe { std::mem::zeroed() };
        if unsafe { libc::tcgetattr(fd, &mut termios) } != 0 {
            return Err(io::Error::last_os_error());
        }
        self.saved = Some(termios);
        let mut raw = termios;
        unsafe { libc::cfmakeraw(&mut raw) };
        if unsafe { libc::tcsetattr(fd, libc::TCSANOW, &raw) } != 0 {
            return Err(io::Error::last_os_error());
        }
        self.raw = true;
        Ok(())
    }

    /// Restore the saved settings.
    pub fn leave_raw(&mut self) {
        if let (true, Some(saved)) = (self.raw, self.saved) {
            unsafe { libc::tcsetattr(self.fd(), libc::TCSANOW, &saved) };
            self.raw = false;
        }
    }

    pub fn size(&self) -> Size {
        if !self.is_tty {
            return Size::default();
        }
        let mut ws: libc::winsize = unsafe { std::mem::zeroed() };
        if unsafe { libc::ioctl(self.fd(), libc::TIOCGWINSZ, &mut ws) } != 0 {
            return Size::default();
        }
        Size {
            cols: ws.ws_col,
            rows: ws.ws_row,
            xpixel: ws.ws_xpixel,
            ypixel: ws.ws_ypixel,
        }
    }

    pub fn write(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.file.write_all(bytes)
    }

    pub fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }

    /// Spawn a background reader that forwards every chunk of stdin-like input.
    ///
    /// The thread lives for the process; the process exits with it.
    pub fn reader(&self) -> io::Result<Receiver<Vec<u8>>> {
        let mut file = self.file.try_clone()?;
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                match file.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        if tx.send(buf[..n].to_vec()).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });
        Ok(rx)
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        self.leave_raw();
    }
}

/// Best-effort screen and image cleanup that runs even on panic.
pub struct Cleanup {
    fd: RawFd,
    visual: bool,
    active: bool,
}

impl Cleanup {
    pub fn new(fd: RawFd, visual: bool) -> Self {
        Self {
            fd,
            visual,
            active: true,
        }
    }
}

impl Drop for Cleanup {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        let delete = crate::kitty::delete_all();
        write_fd(self.fd, &delete);
        if self.visual {
            write_fd(self.fd, b"\x1b[?25h\x1b[?1049l");
        }
    }
}

fn write_fd(fd: RawFd, bytes: &[u8]) {
    let mut offset = 0;
    while offset < bytes.len() {
        let written =
            unsafe { libc::write(fd, bytes[offset..].as_ptr().cast(), bytes.len() - offset) };
        if written <= 0 {
            return;
        }
        offset += written as usize;
    }
}

/// Environment hints a capability decision can consult.
pub fn environment_hints() -> Vec<(&'static str, String)> {
    const KEYS: &[&str] = &[
        "TERM",
        "COLORTERM",
        "TERM_PROGRAM",
        "TERM_PROGRAM_VERSION",
        "KITTY_WINDOW_ID",
        "KITTY_PID",
        "GHOSTTY_RESOURCES_DIR",
        "WEZTERM_EXECUTABLE",
        "KONSOLE_VERSION",
        "TMUX",
        "STY",
        "ZELLIJ",
        "SSH_CONNECTION",
        "SSH_TTY",
    ];
    KEYS.iter()
        .filter_map(|key| std::env::var(key).ok().map(|value| (*key, value)))
        .collect()
}

/// Which multiplexer wrapper a backend must use, if any.
pub fn detect_mux() -> &'static str {
    if std::env::var_os("TMUX").is_some() {
        return "tmux";
    }
    if std::env::var_os("STY").is_some() {
        return "screen";
    }
    if let Ok(term) = std::env::var("TERM")
        && term.starts_with("screen")
    {
        return "screen";
    }
    "none"
}
