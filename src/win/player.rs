use std::{
    fs::File,
    io::{BufRead, BufReader, Write},
    os::windows::io::FromRawHandle,
    thread,
};

use anyhow::{Context, Result, bail};
use windows::Win32::{
    Foundation::{HWND, LPARAM, WPARAM},
    Storage::FileSystem::{CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_GENERIC_READ, FILE_GENERIC_WRITE, OPEN_EXISTING},
    UI::WindowsAndMessaging::{DispatchMessageW, GetMessageW, MSG, PostMessageW, TranslateMessage},
};

use wnd::wide_string::WideString;

use super::video_host::{PLAYER_PAUSE_MESSAGE, PLAYER_RESUME_MESSAGE, PLAYER_STOP_MESSAGE, VideoHost};

pub(crate) fn run_player() -> Result<()> {
    common::logger::init();
    let args = PlayerArgs::parse()?;
    let mut pipe = open_pipe(&args.pipe_name)?;
    let window = VideoHost::create_player_window(&args.source)?;
    let hwnd = window.hwnd();
    writeln!(pipe, "WindowReady {}", hwnd.0 as isize).context("Send WindowReady failed")?;
    start_command_thread(pipe, hwnd);
    run_message_loop()
}

fn start_command_thread(pipe: File, hwnd: HWND) {
    let hwnd = hwnd.0 as isize;
    thread::spawn(move || {
        let mut reader = BufReader::new(pipe);
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => post_command(HWND(hwnd as _), line.trim()),
                Err(error) => {
                    log::error!("Read content command failed: {}", error);
                    break;
                }
            }
        }
        post_command(HWND(hwnd as _), "Stop");
    });
}

fn post_command(hwnd: HWND, command: &str) {
    let message = match command {
        "Pause" => PLAYER_PAUSE_MESSAGE,
        "Resume" => PLAYER_RESUME_MESSAGE,
        "Stop" => PLAYER_STOP_MESSAGE,
        _ => {
            log::error!("Unknown content command: {}", command);
            return;
        }
    };
    unsafe {
        let _ = PostMessageW(Some(hwnd), message, WPARAM(0), LPARAM(0));
    }
}

fn run_message_loop() -> Result<()> {
    let mut message = MSG::default();
    loop {
        let result = unsafe { GetMessageW(&mut message, None, 0, 0) };
        match result.0 {
            0 => return Ok(()),
            -1 => bail!(windows::core::Error::from_thread()),
            _ => unsafe {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            },
        }
    }
}

fn open_pipe(pipe_name: &str) -> Result<File> {
    let pipe_name = WideString::new(pipe_name);
    let handle = unsafe {
        CreateFileW(
            pipe_name.as_pcwstr(),
            FILE_GENERIC_READ.0 | FILE_GENERIC_WRITE.0,
            windows::Win32::Storage::FileSystem::FILE_SHARE_MODE(0),
            None,
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            None,
        )
    }
    .context("Open content pipe failed")?;
    Ok(unsafe { File::from_raw_handle(handle.0 as _) })
}

struct PlayerArgs {
    pipe_name: String,
    source: String,
}

impl PlayerArgs {
    fn parse() -> Result<Self> {
        let mut args = std::env::args().skip(1);
        let mut pipe_name = None;
        let mut source = None;
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--pipe" => pipe_name = args.next(),
                "--source" => source = args.next(),
                _ => bail!("Unknown x-desk-player argument: {}", arg),
            }
        }
        Ok(Self {
            pipe_name: pipe_name.context("Missing --pipe")?,
            source: source.context("Missing --source")?,
        })
    }
}
