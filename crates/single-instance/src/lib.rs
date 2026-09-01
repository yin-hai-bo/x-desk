use std::{
    sync::{
        Arc,
        atomic::AtomicBool,
        mpsc::{self, Receiver, Sender},
    },
    thread,
    time::Duration,
};

use anyhow::{Context, Result};
use windows::{
    Win32::{
        Foundation::{
            CloseHandle, DUPLICATE_SAME_ACCESS, DuplicateHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE,
            WAIT_FAILED, WAIT_OBJECT_0,
        },
        System::Threading::{
            CreateEventW, CreateMutexW, EVENT_MODIFY_STATE, GetCurrentProcess, INFINITE, OpenEventW, SetEvent,
            WaitForMultipleObjects,
        },
    },
    core::{Owned, PCWSTR},
};

pub struct SingleInstance {
    /// 这个对象要放在 single instance 里，保持有效，否则就被 Close 了。
    _mutex: Owned<HANDLE>,

    /// 这个 Event 对象要 Duplicate 一份到等待线程里，主线程和等待线程都要用。
    exit_requested_event: Win32Handle,

    receiver: Option<Receiver<SingleInstanceMessage>>,
    stop_flag: Arc<AtomicBool>,
}

impl Drop for SingleInstance {
    fn drop(&mut self) {
        self.stop_flag.store(true, std::sync::atomic::Ordering::SeqCst);
        let _ = unsafe { SetEvent(*self.exit_requested_event) };
    }
}

pub enum SingleInstanceMessage {
    SecondInstanceStarted,
    ExitRequested,
}

impl SingleInstance {
    pub fn request_exit(name: &str) -> bool {
        notify_event(&core_obj_name(name, "-exit-requested"))
    }

    /// 尝试获取 SingleInstance
    ///
    /// # Parameters
    /// - name 全局内核对象的名字
    ///
    /// # Return
    /// - Ok(None) 表示自己不是第一个实例，已有实例在运行中了，我已经通知了前一实例。
    /// - Ok(Some(SingleInstance)) 表示自己是第一个实例。
    /// - Err 出错了
    pub fn acquire(name: &str) -> Result<Option<Self>> {
        let mutex_name = core_obj_name(name, "");
        let second_instance_started_event_name = core_obj_name(name, "-second-instance-started");
        let exit_requested_event_name = core_obj_name(name, "-exit-requested");

        // 尝试创建 Mutex，若已存在（另一个进程已创建同名对象），则尝试通知前一进程
        let mutex = unsafe {
            Owned::new(
                CreateMutexW(None, true, PCWSTR::from_raw(mutex_name.as_ptr()))
                    .context("Create single-instance mutex failed")?,
            )
        };
        if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
            let _ = notify_event(&second_instance_started_event_name);
            return Ok(None);
        }

        // Mutex 已创建，现在创建两个 Event。
        // 通知第二进程启动的 Event
        let second_instance_started_event = unsafe {
            Win32Handle::new(
                CreateEventW(
                    None,
                    false,
                    false,
                    PCWSTR::from_raw(second_instance_started_event_name.as_ptr()),
                )
                .context("Create single-instance second instance started event failed")?,
            )
        };

        // 通知退出进程的 Event
        let exit_requested_event = unsafe {
            Win32Handle::new(
                CreateEventW(None, false, false, PCWSTR::from_raw(exit_requested_event_name.as_ptr()))
                    .context("Create single-instance exit event failed")?,
            )
        };
        let exit_requested_event_clone = exit_requested_event
            .duplicate()
            .context("Duplicate single-instance event failed")?;

        let (sender, receiver) = mpsc::channel();
        let single_instance = SingleInstance {
            _mutex: mutex,
            exit_requested_event,
            receiver: Some(receiver),
            stop_flag: Arc::new(AtomicBool::new(false)),
        };

        let stop_flag = single_instance.stop_flag.clone();
        thread::spawn(move || {
            wait_for_single_instance_messages(
                stop_flag,
                second_instance_started_event,
                exit_requested_event_clone,
                sender,
            )
        });

        Ok(Some(single_instance))
    }

    pub fn take_message_receiver(&mut self) -> Option<Receiver<SingleInstanceMessage>> {
        self.receiver.take()
    }
}

fn wait_for_single_instance_messages(
    stop_flag: Arc<AtomicBool>,
    second_instance_started_event: Win32Handle,
    exit_requested_event: Win32Handle,
    sender: Sender<SingleInstanceMessage>,
) {
    let handles = [*second_instance_started_event, *exit_requested_event];

    loop {
        let wait_result = unsafe { WaitForMultipleObjects(&handles, false, INFINITE) };
        if stop_flag.load(std::sync::atomic::Ordering::SeqCst) {
            break;
        }

        match wait_result {
            WAIT_OBJECT_0 => {
                let _ = sender.send(SingleInstanceMessage::SecondInstanceStarted);
            }
            result if result.0 == WAIT_OBJECT_0.0 + 1 => {
                let _ = sender.send(SingleInstanceMessage::ExitRequested);
                break;
            }
            WAIT_FAILED => break,
            _ => break,
        }
    }
}

const NOTIFY_RETRY_ATTEMPTS: usize = 20;
const NOTIFY_RETRY_DELAY: Duration = Duration::from_millis(25);

fn notify_event(event_name: &[u16]) -> bool {
    for _ in 0..NOTIFY_RETRY_ATTEMPTS {
        if try_notify_event(event_name) {
            return true;
        }
        thread::sleep(NOTIFY_RETRY_DELAY);
    }
    false
}

fn try_notify_event(event_name: &[u16]) -> bool {
    let event = unsafe {
        Owned::new(OpenEventW(EVENT_MODIFY_STATE, false, PCWSTR::from_raw(event_name.as_ptr())).unwrap_or_default())
    };
    if event.is_invalid() {
        return false;
    }
    match unsafe { SetEvent(*event) } {
        Ok(_) => true,
        Err(_) => false,
    }
}

fn core_obj_name(name: &str, suffix: &str) -> Vec<u16> {
    format!("Local\\{}{}", name, suffix).encode_utf16().chain([0]).collect()
}

struct Win32Handle(HANDLE);
unsafe impl Send for Win32Handle {}

impl Drop for Win32Handle {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            let _ = unsafe { CloseHandle(self.0) };
        }
    }
}

impl From<Win32Handle> for HANDLE {
    fn from(value: Win32Handle) -> Self {
        value.0
    }
}

impl Win32Handle {
    pub unsafe fn new(raw: HANDLE) -> Self {
        Self(raw)
    }

    pub fn duplicate(&self) -> Result<Win32Handle> {
        let mut h: HANDLE = HANDLE::default();
        unsafe {
            DuplicateHandle(
                GetCurrentProcess(),
                self.0,
                GetCurrentProcess(),
                &mut h,
                0,
                false,
                DUPLICATE_SAME_ACCESS,
            )
        }?;
        Ok(unsafe { Self::new(h) })
    }
}

impl std::ops::Deref for Win32Handle {
    type Target = HANDLE;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
