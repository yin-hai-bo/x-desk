use std::{
    os::windows::io::{AsRawHandle, FromRawHandle, HandleOrNull, OwnedHandle},
    sync::{
        Arc,
        atomic::AtomicBool,
        mpsc::{self, Receiver, Sender},
    },
    thread,
    time::Duration,
};

use anyhow::{Context, Result};
use windows_sys::Win32::{
    Foundation::{ERROR_ALREADY_EXISTS, GetLastError, WAIT_OBJECT_0},
    System::Threading::{
        CreateEventW, CreateMutexW, EVENT_MODIFY_STATE, INFINITE, OpenEventW, SetEvent, WaitForSingleObject,
    },
};

pub struct SingleInstance {
    _mutex: OwnedHandle,
    event: OwnedHandle,
    receiver: Option<Receiver<SingleInstanceMessage>>,
    stop_flag: Arc<AtomicBool>,
}

impl Drop for SingleInstance {
    fn drop(&mut self) {
        self.stop_flag.store(true, std::sync::atomic::Ordering::SeqCst);
        let _ = unsafe { SetEvent(self.event.as_raw_handle()) };
    }
}

pub enum SingleInstanceMessage {
    SecondInstanceStarted,
}

impl SingleInstance {
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
        let event_name = core_obj_name(name, "-second-instance-started");

        // 尝试创建 Mutex，若已存在（另一个进程已创建同名对象），则尝试通知前一进程
        let mutex = unsafe { CreateMutexW(std::ptr::null(), 1, mutex_name.as_ptr()) };
        let last_error = unsafe { GetLastError() };
        if mutex.is_null() {
            return Err(std::io::Error::last_os_error()).context("Create single-instance mutex failed");
        }
        let mutex = unsafe { OwnedHandle::from_raw_handle(mutex) };
        if last_error == ERROR_ALREADY_EXISTS {
            notify_existing_instance(&event_name);
            return Ok(None);
        }

        // Mutex 已创建，现在创建 Event。
        let event = OwnedHandle::try_from(unsafe {
            HandleOrNull::from_raw_handle(CreateEventW(std::ptr::null(), 0, 0, event_name.as_ptr()))
        })
        .map_err(|_| std::io::Error::last_os_error())
        .context("Create single-instance notification event failed")?;

        let event_clone = event.try_clone()?;

        let (sender, receiver) = mpsc::channel();
        let single_instance = SingleInstance {
            _mutex: mutex,
            event,
            receiver: Some(receiver),
            stop_flag: Arc::new(AtomicBool::new(false)),
        };

        let stop_flag = single_instance.stop_flag.clone();
        thread::spawn(move || wait_for_second_instance(stop_flag, event_clone, sender));
        Ok(Some(single_instance))
    }

    pub fn take_message_receiver(&mut self) -> Option<Receiver<SingleInstanceMessage>> {
        self.receiver.take()
    }
}

fn wait_for_second_instance(stop_flag: Arc<AtomicBool>, event: OwnedHandle, sender: Sender<SingleInstanceMessage>) {
    loop {
        let wait_result = unsafe { WaitForSingleObject(event.as_raw_handle(), INFINITE) };
        if wait_result == WAIT_OBJECT_0 {
            if stop_flag.load(std::sync::atomic::Ordering::SeqCst) {
                break;
            }
            let _ = sender.send(SingleInstanceMessage::SecondInstanceStarted);
        } else {
            break;
        }
    }
}

const NOTIFY_RETRY_ATTEMPTS: usize = 20;
const NOTIFY_RETRY_DELAY: Duration = Duration::from_millis(25);

fn notify_existing_instance(event_name: &[u16]) {
    for _ in 0..NOTIFY_RETRY_ATTEMPTS {
        if try_notify_existing_instance(event_name) {
            return;
        }
        thread::sleep(NOTIFY_RETRY_DELAY);
    }
}

fn try_notify_existing_instance(event_name: &[u16]) -> bool {
    match OwnedHandle::try_from(unsafe {
        HandleOrNull::from_raw_handle(OpenEventW(EVENT_MODIFY_STATE, 0, event_name.as_ptr()))
    }) {
        Ok(event) => unsafe { SetEvent(event.as_raw_handle()) != 0 },
        Err(_) => false,
    }
}

fn core_obj_name(name: &str, suffix: &str) -> Vec<u16> {
    format!("Local\\{}{}", name, suffix).encode_utf16().chain([0]).collect()
}
