use std::{
    fs::File,
    io::{BufRead, BufReader, Write},
    os::windows::io::FromRawHandle,
    process::{Child, Command},
    sync::atomic::{AtomicU64, Ordering},
};

use anyhow::{Context, Result, anyhow, bail};
use windows::Win32::{
    Foundation::{ERROR_PIPE_CONNECTED, GetLastError, HANDLE, HWND},
    Storage::FileSystem::{FILE_FLAG_FIRST_PIPE_INSTANCE, PIPE_ACCESS_DUPLEX},
    System::Pipes::{ConnectNamedPipe, CreateNamedPipeW, PIPE_READMODE_BYTE, PIPE_TYPE_BYTE, PIPE_WAIT},
    UI::WindowsAndMessaging::GetWindowThreadProcessId,
};

use crate::{
    config::{WallpaperContentSpec, WallpaperKind},
    win::wide_string::WideString,
};

const PIPE_BUFFER_SIZE: u32 = 4096;
static NEXT_PIPE_ID: AtomicU64 = AtomicU64::new(1);

pub(super) enum ContentCommand {
    Pause,
    Resume,
    Stop,
}

pub(super) struct ContentProcessHandle {
    process: Child,
    pipe: File,
    hwnd: HWND,
}

impl ContentProcessHandle {
    pub(super) fn process_id(&self) -> u32 {
        self.process.id()
    }

    pub(super) fn hwnd(&self) -> HWND {
        self.hwnd
    }

    pub(super) fn is_running(&mut self) -> Result<bool> {
        Ok(self
            .process
            .try_wait()
            .context("Check content process status failed")?
            .is_none())
    }

    pub(super) fn send_command(&mut self, command: ContentCommand) -> Result<()> {
        let command = match command {
            ContentCommand::Pause => "Pause",
            ContentCommand::Resume => "Resume",
            ContentCommand::Stop => "Stop",
        };
        writeln!(self.pipe, "{}", command).context("Write content command failed")
    }
}

impl Drop for ContentProcessHandle {
    fn drop(&mut self) {
        let _ = self.send_command(ContentCommand::Stop);
        let _ = self.process.kill();
    }
}

pub(super) fn start_content_process(content: &WallpaperContentSpec) -> Result<ContentProcessHandle> {
    match content.kind {
        WallpaperKind::Video => start_video_process(content),
        WallpaperKind::WebView => start_webview_process(content),
    }
}

fn start_video_process(content: &WallpaperContentSpec) -> Result<ContentProcessHandle> {
    start_process(content, "x-desk-player.exe", "player")
}

fn start_webview_process(content: &WallpaperContentSpec) -> Result<ContentProcessHandle> {
    start_process(content, "x-desk-webview.exe", "webview")
}

fn start_process(content: &WallpaperContentSpec, exe_name: &str, pipe_label: &str) -> Result<ContentProcessHandle> {
    let pipe_name = format!(
        r"\\.\pipe\x-desk-{}-{}-{}",
        pipe_label,
        std::process::id(),
        NEXT_PIPE_ID.fetch_add(1, Ordering::Relaxed)
    );
    let pipe = create_pipe_server(&pipe_name)?;
    let process = Command::new(content_exe_path(exe_name)?)
        .arg("--pipe")
        .arg(&pipe_name)
        .arg("--source")
        .arg(&content.source)
        .spawn()
        .with_context(|| format!("Start {} content process failed", pipe_label))?;

    connect_pipe(pipe.as_handle())?;

    let mut reader = BufReader::new(pipe.try_clone().context("Clone content pipe failed")?);
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .context("Read WindowReady from content process failed")?;
    let hwnd = parse_window_ready(&line)?;
    verify_hwnd_owner(hwnd, process.id())?;

    Ok(ContentProcessHandle { process, pipe, hwnd })
}

fn create_pipe_server(pipe_name: &str) -> Result<File> {
    let pipe_name = WideString::new(pipe_name);
    let handle = unsafe {
        CreateNamedPipeW(
            pipe_name.as_pcwstr(),
            PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE,
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
            1,
            PIPE_BUFFER_SIZE,
            PIPE_BUFFER_SIZE,
            0,
            None,
        )
    };
    if handle.is_invalid() {
        return Err(windows::core::Error::from_thread()).context("Create named pipe failed");
    }

    Ok(unsafe { File::from_raw_handle(handle.0 as _) })
}

fn connect_pipe(handle: HANDLE) -> Result<()> {
    match unsafe { ConnectNamedPipe(handle, None) } {
        Ok(_) => Ok(()),
        Err(_) if unsafe { GetLastError() } == ERROR_PIPE_CONNECTED => Ok(()),
        Err(error) => Err(error).context("Connect named pipe failed"),
    }
}

fn parse_window_ready(line: &str) -> Result<HWND> {
    let value = line
        .trim()
        .strip_prefix("WindowReady ")
        .ok_or_else(|| anyhow!("Unexpected content process message: {}", line.trim()))?;
    let hwnd = value.parse::<isize>().context("Parse content HWND failed")?;
    Ok(HWND(hwnd as _))
}

fn verify_hwnd_owner(hwnd: HWND, expected_process_id: u32) -> Result<()> {
    let mut actual_process_id = 0;
    unsafe {
        GetWindowThreadProcessId(hwnd, Some(&mut actual_process_id));
    }
    if actual_process_id != expected_process_id {
        bail!(
            "Content HWND belongs to process {}, expected {}",
            actual_process_id,
            expected_process_id
        );
    }
    Ok(())
}

fn content_exe_path(exe_name: &str) -> Result<std::path::PathBuf> {
    Ok(std::env::current_exe()?.with_file_name(exe_name))
}

trait PipeHandle {
    fn as_handle(&self) -> HANDLE;
}

impl PipeHandle for File {
    fn as_handle(&self) -> HANDLE {
        use std::os::windows::io::AsRawHandle;

        HANDLE(self.as_raw_handle())
    }
}
