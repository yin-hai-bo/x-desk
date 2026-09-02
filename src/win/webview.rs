use std::{
    collections::HashMap,
    fs::File,
    io::{BufRead, BufReader, Write},
    os::windows::io::FromRawHandle,
    path::Path,
    sync::Mutex,
    thread,
};

use anyhow::{Context, Result, bail};
use regex::Regex;
use webview2_com::{Microsoft::Web::WebView2::Win32::*, *};
use windows::{
    Win32::{
        Foundation::{E_POINTER, HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM},
        Storage::FileSystem::{
            CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_GENERIC_READ, FILE_GENERIC_WRITE, OPEN_EXISTING,
        },
        System::{
            Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize},
            LibraryLoader::GetModuleHandleW,
        },
        UI::{
            HiDpi::{DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext},
            WindowsAndMessaging::{
                CS_HREDRAW, CS_VREDRAW, DefWindowProcW, DispatchMessageW, GetClientRect, GetMessageW, MSG,
                PostMessageW, PostQuitMessage, RegisterClassExW, TranslateMessage, WINDOW_EX_STYLE, WM_DESTROY,
                WM_NCCREATE, WM_NCDESTROY, WM_SIZE, WNDCLASSEXW, WS_CLIPCHILDREN, WS_CLIPSIBLINGS, WS_POPUP,
                WS_VISIBLE,
            },
        },
    },
    core::{PCWSTR, w},
};
use windows_core::Interface;
use wnd::Window;

use wnd::wide_string::WideString;

const WEBVIEW_WINDOW_CLASS_NAME: PCWSTR = w!("X-Desk-WebView-Class");
const WEBVIEW_STOP_MESSAGE: u32 = windows::Win32::UI::WindowsAndMessaging::WM_APP + 1;
const WEBVIEW_PAUSE_MESSAGE: u32 = windows::Win32::UI::WindowsAndMessaging::WM_APP + 2;
const WEBVIEW_RESUME_MESSAGE: u32 = windows::Win32::UI::WindowsAndMessaging::WM_APP + 3;
static WEBVIEW_WINDOW_CLASS_REGISTERED: Mutex<bool> = Mutex::new(false);

const INSTALL_MEDIA_CONTROL_SCRIPT: &str = r#"
(() => {
  if (window.__xDeskMediaControlInstalled) {
    return;
  }

  window.__xDeskMediaControlInstalled = true;
  window.__xDeskOccluded = false;

  const pauseManagedVideos = (root) => {
    root.querySelectorAll?.('video').forEach((video) => {
      if (!video.paused) {
        video.dataset.xDeskPausedForOcclusion = 'true';
        video.pause();
      }
    });
  };

  const resumeManagedVideos = (root) => {
    root
      .querySelectorAll?.('video[data-x-desk-paused-for-occlusion="true"]')
      .forEach((video) => {
        delete video.dataset.xDeskPausedForOcclusion;
        video.play().catch(() => {});
      });
  };

  const propagateOcclusion = (occluded) => {
    document.querySelectorAll('iframe').forEach((frame) => {
      frame.contentWindow?.postMessage(
        { type: 'x-desk-set-occluded', occluded },
        '*'
      );
    });
  };

  window.__xDeskSetOccluded = (occluded) => {
    window.__xDeskOccluded = !!occluded;
    if (window.__xDeskOccluded) {
      pauseManagedVideos(document);
    } else {
      resumeManagedVideos(document);
    }
    propagateOcclusion(window.__xDeskOccluded);
  };

  document.addEventListener(
    'play',
    (event) => {
      if (!window.__xDeskOccluded) {
        return;
      }
      if (event.target instanceof HTMLVideoElement) {
        event.target.dataset.xDeskPausedForOcclusion = 'true';
        event.target.pause();
      }
    },
    true
  );

  new MutationObserver((mutations) => {
    if (!window.__xDeskOccluded) {
      return;
    }
    for (const mutation of mutations) {
      mutation.addedNodes.forEach((node) => {
        if (!(node instanceof Element)) {
          return;
        }
        if (node instanceof HTMLVideoElement) {
          pauseManagedVideos(node.parentElement ?? document);
          return;
        }
        pauseManagedVideos(node);
      });
    }
  }).observe(document.documentElement, { childList: true, subtree: true });

  window.addEventListener('message', (event) => {
    if (event.data?.type === 'x-desk-set-occluded') {
      window.__xDeskSetOccluded(event.data.occluded);
    }
  });
})();
"#;

const PAUSE_VIDEOS_SCRIPT: &str = r#"
(() => {
  window.__xDeskSetOccluded?.(true);
})();
"#;

const RESUME_VIDEOS_SCRIPT: &str = r#"
(() => {
  window.__xDeskSetOccluded?.(false);
})();
"#;

pub(crate) fn run_webview() -> Result<()> {
    common::logger::init();
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
    let _com = ComApartment::init()?;
    let args = WebViewArgs::parse()?;
    let mut pipe = open_pipe(&args.pipe_name)?;
    let mut window = WebViewWindow::create(&args.source)?;
    let hwnd = window.hwnd();
    window.component_mut().initialize(hwnd)?;
    writeln!(pipe, "WindowReady {}", hwnd.0 as isize).context("Send WindowReady failed")?;
    start_command_thread(pipe, hwnd, args.respond_to_media_control_commands);
    run_message_loop()
}

struct WebViewWindow {
    source: String,
    virtual_host_mappings: Vec<VirtualHostMapping>,
    controller: Option<ICoreWebView2Controller>,
    webview: Option<ICoreWebView2>,
}

impl WebViewWindow {
    fn create(source: &str) -> Result<Box<Window<Self>>> {
        let inst = unsafe { GetModuleHandleW(PCWSTR::null()) }?.into();
        Self::register_class(inst)?;
        let source = WebViewSource::from_config(source)?;
        Window::create(
            WINDOW_EX_STYLE(0),
            WEBVIEW_WINDOW_CLASS_NAME,
            WideString::new("x-desk-webview").as_pcwstr(),
            WS_POPUP | WS_VISIBLE | WS_CLIPCHILDREN | WS_CLIPSIBLINGS,
            0,
            0,
            1,
            1,
            None,
            None,
            Some(inst),
            Self {
                source: source.content,
                virtual_host_mappings: source.virtual_host_mappings,
                controller: None,
                webview: None,
            },
        )
    }

    fn initialize(&mut self, hwnd: HWND) -> Result<()> {
        let environment = create_environment()?;
        let controller = create_controller(&environment, hwnd)?;
        let webview = unsafe { controller.CoreWebView2() }.context("Get CoreWebView2 failed")?;
        unsafe {
            if let Ok(settings) = webview.Settings() {
                let _ = settings.SetAreDefaultContextMenusEnabled(false);
                let _ = settings.SetAreDevToolsEnabled(false);
            }
            self.apply_virtual_host_mappings(&webview)?;
            self.install_media_control_script(&webview)?;
            controller
                .SetIsVisible(true)
                .context("Show WebView2 controller failed")?;
        }
        self.controller = Some(controller);
        self.webview = Some(webview);
        self.resize(hwnd)?;
        self.navigate()
    }

    fn navigate(&self) -> Result<()> {
        let webview = self.webview.as_ref().context("WebView2 is not initialized")?;
        let source = CoTaskMemPWSTR::from(self.source.as_str());
        unsafe {
            if is_inline_html(&self.source) {
                webview.NavigateToString(*source.as_ref().as_pcwstr())
            } else {
                webview.Navigate(*source.as_ref().as_pcwstr())
            }
        }
        .context("Navigate WebView2 failed")
    }

    fn pause_videos_for_occlusion(&self) -> Result<()> {
        self.execute_script(PAUSE_VIDEOS_SCRIPT)
            .context("Pause WebView videos failed")?;
        #[cfg(debug_assertions)]
        log::debug!("Executed WebView video pause script");
        Ok(())
    }

    fn resume_videos_from_occlusion(&self) -> Result<()> {
        self.execute_script(RESUME_VIDEOS_SCRIPT)
            .context("Resume WebView videos failed")?;
        #[cfg(debug_assertions)]
        log::debug!("Executed WebView video resume script");
        Ok(())
    }

    fn execute_script(&self, script: &str) -> Result<()> {
        let webview = self.webview.as_ref().context("WebView2 is not initialized")?.clone();
        let script = script.to_string();
        ExecuteScriptCompletedHandler::wait_for_async_operation(
            Box::new(move |handler| unsafe {
                let script = CoTaskMemPWSTR::from(script.as_str());
                webview
                    .ExecuteScript(*script.as_ref().as_pcwstr(), &handler)
                    .map_err(webview2_com::Error::WindowsError)
            }),
            Box::new(|error_code, _result| error_code),
        )?;
        Ok(())
    }

    unsafe fn install_media_control_script(&self, webview: &ICoreWebView2) -> Result<()> {
        let webview = webview.clone();
        let script = INSTALL_MEDIA_CONTROL_SCRIPT.to_string();
        AddScriptToExecuteOnDocumentCreatedCompletedHandler::wait_for_async_operation(
            Box::new(move |handler| unsafe {
                let script = CoTaskMemPWSTR::from(script.as_str());
                webview
                    .AddScriptToExecuteOnDocumentCreated(*script.as_ref().as_pcwstr(), &handler)
                    .map_err(webview2_com::Error::WindowsError)
            }),
            Box::new(|error_code, _script_id| error_code),
        )?;
        Ok(())
    }

    unsafe fn apply_virtual_host_mappings(&self, webview: &ICoreWebView2) -> Result<()> {
        if self.virtual_host_mappings.is_empty() {
            return Ok(());
        }
        let webview3: ICoreWebView2_3 = webview.cast().context("Get ICoreWebView2_3 failed")?;
        for mapping in &self.virtual_host_mappings {
            let host = CoTaskMemPWSTR::from(mapping.host.as_str());
            let folder = CoTaskMemPWSTR::from(mapping.folder.as_str());
            unsafe {
                webview3.SetVirtualHostNameToFolderMapping(
                    *host.as_ref().as_pcwstr(),
                    *folder.as_ref().as_pcwstr(),
                    COREWEBVIEW2_HOST_RESOURCE_ACCESS_KIND_ALLOW,
                )
            }
            .with_context(|| format!("Set WebView2 virtual host mapping failed for {}", mapping.host))?;
        }
        Ok(())
    }

    fn resize(&self, hwnd: HWND) -> Result<()> {
        let Some(controller) = self.controller.as_ref() else {
            return Ok(());
        };
        let mut rect = RECT::default();
        unsafe { GetClientRect(hwnd, &mut rect) }.context("Get WebView window client rect failed")?;
        unsafe { controller.SetBounds(rect) }.context("Resize WebView2 controller failed")
    }

    fn register_class(inst: HINSTANCE) -> Result<()> {
        let mut registered = WEBVIEW_WINDOW_CLASS_REGISTERED.lock().unwrap();
        if *registered {
            return Ok(());
        }
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(Self::window_proc),
            hInstance: inst,
            lpszClassName: WEBVIEW_WINDOW_CLASS_NAME,
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
            WM_NCCREATE => Window::<WebViewWindow>::on_wm_nccreate(hwnd, lparam),
            WM_NCDESTROY => Window::<WebViewWindow>::on_wm_ncdestroy(hwnd),
            WM_SIZE => {
                if let Some(ptr) = Window::<WebViewWindow>::get_self_from_hwnd(hwnd) {
                    if let Err(e) = unsafe { ptr.as_ref() }.component().resize(hwnd) {
                        log::error!("Resize WebView wallpaper failed: {}", e);
                    }
                }
                return LRESULT(0);
            }
            WEBVIEW_PAUSE_MESSAGE => {
                #[cfg(debug_assertions)]
                log::debug!("Handle WebView pause message, hwnd={:?}", hwnd);
                if let Some(ptr) = Window::<WebViewWindow>::get_self_from_hwnd(hwnd) {
                    if let Err(e) = unsafe { ptr.as_ref() }.component().pause_videos_for_occlusion() {
                        log::error!("Pause WebView videos failed: {}", e);
                    }
                }
                return LRESULT(0);
            }
            WEBVIEW_RESUME_MESSAGE => {
                #[cfg(debug_assertions)]
                log::debug!("Handle WebView resume message, hwnd={:?}", hwnd);
                if let Some(ptr) = Window::<WebViewWindow>::get_self_from_hwnd(hwnd) {
                    if let Err(e) = unsafe { ptr.as_ref() }.component().resume_videos_from_occlusion() {
                        log::error!("Resume WebView videos failed: {}", e);
                    }
                }
                return LRESULT(0);
            }
            WM_DESTROY | WEBVIEW_STOP_MESSAGE => {
                unsafe { PostQuitMessage(0) };
                return LRESULT(0);
            }
            _ => {}
        }
        unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
    }
}

fn create_environment() -> Result<ICoreWebView2Environment> {
    let (tx, rx) = std::sync::mpsc::channel();
    CreateCoreWebView2EnvironmentCompletedHandler::wait_for_async_operation(
        Box::new(|handler| unsafe {
            CreateCoreWebView2Environment(&handler).map_err(webview2_com::Error::WindowsError)
        }),
        Box::new(move |error_code, environment| {
            error_code?;
            tx.send(environment.ok_or_else(|| windows::core::Error::from(E_POINTER)))
                .expect("send WebView2 environment");
            Ok(())
        }),
    )?;
    Ok(rx.recv().context("Receive WebView2 environment failed")??)
}

fn create_controller(environment: &ICoreWebView2Environment, hwnd: HWND) -> Result<ICoreWebView2Controller> {
    let (tx, rx) = std::sync::mpsc::channel();
    let environment = environment.clone();
    CreateCoreWebView2ControllerCompletedHandler::wait_for_async_operation(
        Box::new(move |handler| unsafe {
            environment
                .CreateCoreWebView2Controller(hwnd, &handler)
                .map_err(webview2_com::Error::WindowsError)
        }),
        Box::new(move |error_code, controller| {
            error_code?;
            tx.send(controller.ok_or_else(|| windows::core::Error::from(E_POINTER)))
                .expect("send WebView2 controller");
            Ok(())
        }),
    )?;
    Ok(rx.recv().context("Receive WebView2 controller failed")??)
}

fn start_command_thread(pipe: File, hwnd: HWND, respond_to_media_control_commands: bool) {
    let hwnd = hwnd.0 as isize;
    thread::spawn(move || {
        let mut reader = BufReader::new(pipe);
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => {
                    let command = line.trim();
                    #[cfg(debug_assertions)]
                    log::debug!("Received WebView content command: {}", command);
                    post_command(HWND(hwnd as _), command, respond_to_media_control_commands);
                }
                Err(error) => {
                    log::error!("Read content command failed: {}", error);
                    break;
                }
            }
        }
        post_command(HWND(hwnd as _), "Stop", true);
    });
}

fn post_command(hwnd: HWND, command: &str, respond_to_media_control_commands: bool) {
    if !respond_to_media_control_commands && matches!(command, "Pause" | "Resume") {
        #[cfg(debug_assertions)]
        log::debug!("Ignored WebView media control command: {}", command);
        return;
    }

    match command {
        "Pause" => unsafe {
            #[cfg(debug_assertions)]
            log::debug!("Post WebView pause message, hwnd={:?}", hwnd);
            let _ = PostMessageW(Some(hwnd), WEBVIEW_PAUSE_MESSAGE, WPARAM(0), LPARAM(0));
        },
        "Resume" => unsafe {
            #[cfg(debug_assertions)]
            log::debug!("Post WebView resume message, hwnd={:?}", hwnd);
            let _ = PostMessageW(Some(hwnd), WEBVIEW_RESUME_MESSAGE, WPARAM(0), LPARAM(0));
        },
        "Stop" => unsafe {
            let _ = PostMessageW(Some(hwnd), WEBVIEW_STOP_MESSAGE, WPARAM(0), LPARAM(0));
        },
        _ => log::error!("Unknown content command: {}", command),
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

struct WebViewSource {
    content: String,
    virtual_host_mappings: Vec<VirtualHostMapping>,
}

struct VirtualHostMapping {
    host: String,
    folder: String,
}

impl WebViewSource {
    fn from_config(source: &str) -> Result<Self> {
        let trimmed = source.trim();
        if trimmed.is_empty() {
            bail!("WebView source is empty");
        }
        if is_inline_html(trimmed) {
            let (content, virtual_host_mappings) = rewrite_file_urls_to_virtual_hosts(trimmed)?;
            return Ok(Self {
                content,
                virtual_host_mappings,
            });
        }
        if trimmed.starts_with("http://")
            || trimmed.starts_with("https://")
            || trimmed.starts_with("file://")
            || trimmed.starts_with("data:")
        {
            return Ok(Self {
                content: trimmed.to_string(),
                virtual_host_mappings: Vec::new(),
            });
        }
        Ok(Self {
            content: path_to_file_url(
                &Path::new(trimmed)
                    .canonicalize()
                    .context("Resolve WebView source path failed")?
                    .to_string_lossy(),
            ),
            virtual_host_mappings: Vec::new(),
        })
    }
}

fn rewrite_file_urls_to_virtual_hosts(html: &str) -> Result<(String, Vec<VirtualHostMapping>)> {
    let re = Regex::new(r#"file:///[^"][^\s<>\"]*"#).context("Build file URL regex failed")?;
    let mut mappings = Vec::new();
    let mut folder_hosts = HashMap::<String, String>::new();
    let mut rewritten = String::with_capacity(html.len());
    let mut last = 0;

    for matched in re.find_iter(html) {
        rewritten.push_str(&html[last..matched.start()]);
        let file_url = matched.as_str();
        let file_path = file_url_to_windows_path(file_url)?;
        let folder = file_path
            .parent()
            .context("WebView file URL has no parent directory")?
            .canonicalize()
            .context("Resolve WebView asset folder failed")?;
        let folder_key = folder.to_string_lossy().into_owned();
        let host = match folder_hosts.get(&folder_key) {
            Some(host) => host.clone(),
            None => {
                let host = format!("x-desk-assets-{}.local", folder_hosts.len() + 1);
                folder_hosts.insert(folder_key.clone(), host.clone());
                mappings.push(VirtualHostMapping {
                    host: host.clone(),
                    folder: folder_key,
                });
                host
            }
        };
        let file_name = file_path
            .file_name()
            .context("WebView file URL has no file name")?
            .to_string_lossy()
            .replace('\\', "/");
        rewritten.push_str(&format!("https://{}/{}", host, file_name));
        last = matched.end();
    }

    rewritten.push_str(&html[last..]);
    Ok((rewritten, mappings))
}

fn file_url_to_windows_path(file_url: &str) -> Result<std::path::PathBuf> {
    let path = file_url
        .strip_prefix("file:///")
        .context("Unsupported WebView file URL")?
        .replace("%20", " ")
        .replace('/', "\\");
    Ok(std::path::PathBuf::from(path))
}

fn is_inline_html(source: &str) -> bool {
    let trimmed = source.trim_start();
    trimmed.starts_with("<!doctype html") || trimmed.starts_with("<!DOCTYPE html") || trimmed.starts_with("<html")
}

fn path_to_file_url(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    if let Some(rest) = normalized.strip_prefix("//") {
        format!("file://{}", rest)
    } else {
        format!("file:///{}", normalized)
    }
}

struct WebViewArgs {
    pipe_name: String,
    source: String,
    respond_to_media_control_commands: bool,
}

impl WebViewArgs {
    fn parse() -> Result<Self> {
        let mut args = std::env::args().skip(1);
        let mut pipe_name = None;
        let mut source = None;
        let mut respond_to_media_control_commands = true;
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--pipe" => pipe_name = args.next(),
                "--source" => source = args.next(),
                "--respond-to-media-control-commands" => {
                    respond_to_media_control_commands = parse_bool_arg(
                        args.next()
                            .context("Missing --respond-to-media-control-commands value")?
                            .as_str(),
                    )?
                }
                _ => bail!("Unknown x-desk-webview argument: {}", arg),
            }
        }
        Ok(Self {
            pipe_name: pipe_name.context("Missing --pipe")?,
            source: source.context("Missing --source")?,
            respond_to_media_control_commands,
        })
    }
}

fn parse_bool_arg(value: &str) -> Result<bool> {
    match value {
        "true" | "1" | "yes" | "on" => Ok(true),
        "false" | "0" | "no" | "off" => Ok(false),
        _ => bail!("Invalid boolean value: {}", value),
    }
}

struct ComApartment;

impl ComApartment {
    fn init() -> Result<Self> {
        unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }
            .ok()
            .context("CoInitializeEx failed")?;
        Ok(Self)
    }
}

impl Drop for ComApartment {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}
