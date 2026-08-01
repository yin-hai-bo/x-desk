use std::{
    ops::{Deref, DerefMut},
    path::Path,
    sync::{Mutex, OnceLock},
};

use anyhow::{Context, Result, bail};
use windows::{
    Win32::{
        Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM},
        Graphics::Gdi::{BLACK_BRUSH, GetStockObject, HBRUSH},
        Media::MediaFoundation::{
            CLSID_MFMediaEngineClassFactory, IMFAttributes, IMFMediaEngine, IMFMediaEngineClassFactory,
            IMFMediaEngineNotify, IMFMediaEngineNotify_Impl, MF_MEDIA_ENGINE_CALLBACK, MF_MEDIA_ENGINE_EVENT_ERROR,
            MF_MEDIA_ENGINE_EVENT_PLAYING, MF_MEDIA_ENGINE_PLAYBACK_HWND, MF_VERSION, MFCreateAttributes,
            MFSTARTUP_FULL, MFStartup,
        },
        System::{
            Com::{CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize},
            LibraryLoader::GetModuleHandleW,
        },
        UI::WindowsAndMessaging::{
            DefWindowProcW, GetClientRect, RegisterClassExW, SWP_NOACTIVATE, SWP_NOZORDER, SWP_SHOWWINDOW,
            WINDOW_EX_STYLE, WM_NCCREATE, WM_NCDESTROY, WNDCLASSEXW, WS_CHILD, WS_CLIPCHILDREN, WS_CLIPSIBLINGS,
            WS_VISIBLE,
        },
    },
    core::{BSTR, PCWSTR, w},
};
use windows_implement::implement;

use crate::win::{wide_string::WideString, win_utils, window::Window};

const VIDEO_HOST_CLASS_NAME: PCWSTR = w!("X-Desk-VideoHost-Class");
static VIDEO_HOST_CLASS_REGISTERED: Mutex<bool> = Mutex::new(false);
static MEDIA_FOUNDATION_STARTED: OnceLock<std::result::Result<(), String>> = OnceLock::new();

pub(super) struct VideoHost {
    source_url: String,
    player: Option<VideoPlayer>,
}

impl VideoHost {
    pub fn create(parent: HWND, source: &str) -> Result<Box<Window<Self>>> {
        let source_url = local_video_source_url(source)?;
        let inst = unsafe { GetModuleHandleW(PCWSTR::null()) }?.into();
        Self::register_class(inst)?;

        let rect = parent_client_rect(parent)?;
        let mut window = Window::create(
            WINDOW_EX_STYLE(0),
            VIDEO_HOST_CLASS_NAME,
            WideString::new("x-desk-video-host").as_pcwstr(),
            WS_CHILD | WS_VISIBLE | WS_CLIPCHILDREN | WS_CLIPSIBLINGS,
            0,
            0,
            win_utils::width_of_rect(&rect),
            win_utils::height_of_rect(&rect),
            Some(parent),
            None,
            Some(inst),
            Self {
                source_url,
                player: None,
            },
        )?;

        let hwnd = window.hwnd();
        window.component_mut().play(hwnd)?;
        Ok(window)
    }

    pub fn set_source(&mut self, hwnd: HWND, source: &str) -> Result<()> {
        let source_url = local_video_source_url(source)?;
        if self.source_url == source_url {
            return Ok(());
        }
        self.source_url = source_url;
        self.play(hwnd)
    }

    pub fn resize_to_parent(&self, hwnd: HWND, parent: HWND) -> Result<()> {
        let rect = parent_client_rect(parent)?;
        win_utils::set_window_pos(
            hwnd,
            None,
            0,
            0,
            win_utils::width_of_rect(&rect),
            win_utils::height_of_rect(&rect),
            SWP_SHOWWINDOW | SWP_NOACTIVATE | SWP_NOZORDER,
        )
    }

    fn play(&mut self, hwnd: HWND) -> Result<()> {
        self.player = None;
        self.player = Some(VideoPlayer::create(hwnd, &self.source_url)?);
        Ok(())
    }

    fn register_class(inst: HINSTANCE) -> Result<()> {
        let mut registered = VIDEO_HOST_CLASS_REGISTERED.lock().unwrap();
        if *registered {
            return Ok(());
        }

        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(Self::window_proc),
            hInstance: inst,
            lpszClassName: VIDEO_HOST_CLASS_NAME,
            hbrBackground: unsafe { HBRUSH(GetStockObject(BLACK_BRUSH).0) },
            ..Default::default()
        };
        let atom = unsafe { RegisterClassExW(&wc) };
        if atom == 0 {
            return Err(windows::core::Error::from_thread().into());
        }
        *registered = true;
        Ok(())
    }

    unsafe extern "system" fn window_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        match msg {
            WM_NCCREATE => Window::<VideoHost>::on_wm_nccreate(hwnd, lparam),
            WM_NCDESTROY => Window::<VideoHost>::on_wm_ncdestroy(hwnd),
            _ => {}
        }
        unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
    }
}

impl Deref for Window<VideoHost> {
    type Target = VideoHost;

    fn deref(&self) -> &Self::Target {
        self.component()
    }
}

impl DerefMut for Window<VideoHost> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.component_mut()
    }
}

struct VideoPlayer {
    engine: IMFMediaEngine,
    _notify: IMFMediaEngineNotify,
    _com: ComApartment,
}

impl VideoPlayer {
    fn create(hwnd: HWND, source_url: &str) -> Result<Self> {
        start_media_foundation()?;
        let com = ComApartment::init()?;
        let notify: IMFMediaEngineNotify = MediaEngineNotify.into();

        let mut attributes: Option<IMFAttributes> = None;
        unsafe { MFCreateAttributes(&mut attributes, 2) }.context("MFCreateAttributes() failed")?;
        let attributes = attributes.context("MFCreateAttributes() returned no attributes")?;
        unsafe {
            attributes
                .SetUnknown(&MF_MEDIA_ENGINE_CALLBACK, &notify)
                .context("Set media engine callback failed")?;
            attributes
                .SetUINT64(&MF_MEDIA_ENGINE_PLAYBACK_HWND, hwnd.0 as u64)
                .context("Set media engine playback window failed")?;
        }

        let factory: IMFMediaEngineClassFactory =
            unsafe { CoCreateInstance(&CLSID_MFMediaEngineClassFactory, None, CLSCTX_INPROC_SERVER) }
                .context("Create MFMediaEngineClassFactory failed")?;
        let engine = unsafe { factory.CreateInstance(0, &attributes) }.context("Create media engine failed")?;
        let source = BSTR::from(source_url);
        unsafe {
            engine.SetLoop(true).context("Set media engine loop failed")?;
            engine.SetMuted(true).context("Mute media engine failed")?;
            engine.SetSource(&source).context("Set media source failed")?;
            engine.Play().context("Start media playback failed")?;
        }

        Ok(Self {
            engine,
            _notify: notify,
            _com: com,
        })
    }
}

impl Drop for VideoPlayer {
    fn drop(&mut self) {
        unsafe {
            let _ = self.engine.Pause();
            let _ = self.engine.Shutdown();
        }
    }
}

struct ComApartment {
    initialized: bool,
}

impl ComApartment {
    fn init() -> Result<Self> {
        let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        hr.ok().context("CoInitializeEx() failed")?;
        Ok(Self { initialized: true })
    }
}

impl Drop for ComApartment {
    fn drop(&mut self) {
        if self.initialized {
            unsafe { CoUninitialize() };
        }
    }
}

#[implement(IMFMediaEngineNotify)]
struct MediaEngineNotify;

#[allow(non_snake_case)]
impl IMFMediaEngineNotify_Impl for MediaEngineNotify_Impl {
    fn EventNotify(&self, event: u32, _param1: usize, param2: u32) -> windows::core::Result<()> {
        match event {
            event if event == MF_MEDIA_ENGINE_EVENT_ERROR.0 as u32 => {
                log::error!("Media engine playback error: {}", param2);
            }
            event if event == MF_MEDIA_ENGINE_EVENT_PLAYING.0 as u32 => {
                log::info!("Media engine playback started");
            }
            _ => {}
        }
        Ok(())
    }
}

fn start_media_foundation() -> Result<()> {
    let result = MEDIA_FOUNDATION_STARTED
        .get_or_init(|| unsafe { MFStartup(MF_VERSION, MFSTARTUP_FULL) }.map_err(|e| e.to_string()));
    result
        .as_ref()
        .map(|_| ())
        .map_err(|error| anyhow::anyhow!(error.clone()))
}

fn parent_client_rect(parent: HWND) -> Result<RECT> {
    let mut rect = RECT::default();
    unsafe { GetClientRect(parent, &mut rect) }.context("GetClientRect() failed")?;
    Ok(rect)
}

fn local_video_source_url(source: &str) -> Result<String> {
    let trimmed = source.trim();
    if trimmed.is_empty() {
        bail!("Video source is empty");
    }

    let path = Path::new(trimmed);
    if !path.exists() {
        bail!("Video source does not exist: {}", trimmed);
    }

    let source_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };

    Ok(path_to_file_url(&source_path.to_string_lossy()))
}

fn path_to_file_url(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    if let Some(rest) = normalized.strip_prefix("//") {
        format!("file://{}", rest)
    } else {
        format!("file:///{}", normalized)
    }
}
