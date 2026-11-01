//! OS mode changes run on a worker, never in the playback/control event loop.
use std::{
    ffi::c_void,
    sync::{Arc, Condvar, Mutex, mpsc},
    thread,
};
unsafe extern "C" {
    fn cine_display_identity(hwnd: usize, out: *mut u16, cap: i32) -> i32;
    fn cine_display_create() -> *mut c_void;
    fn cine_display_update(context: *mut c_void, hwnd: usize, enabled: i32, fps: f64) -> i32;
    fn cine_display_destroy(context: *mut c_void);
}
pub fn identity(hwnd: usize) -> String {
    let mut text = [0_u16; 256];
    let n = unsafe { cine_display_identity(hwnd, text.as_mut_ptr(), text.len() as i32) };
    if n <= 0 {
        String::new()
    } else {
        String::from_utf16_lossy(&text[..(n as usize).min(text.len())])
    }
}
struct Worker {
    pending: Arc<Mailbox>,
    thread: Option<thread::JoinHandle<()>>,
    reply: mpsc::Receiver<i32>,
}
#[derive(Clone, Copy, Debug, PartialEq)]
enum Request {
    Mode(usize, bool, f64),
    Stop,
}
#[derive(Default)]
struct Mailbox {
    pending: Mutex<Option<Request>>,
    wake: Condvar,
}
impl Mailbox {
    fn post(&self, request: Request) {
        *self.pending.lock().unwrap_or_else(|p| p.into_inner()) = Some(request);
        self.wake.notify_one();
    }
    fn next(&self) -> Request {
        let mut pending = self.pending.lock().unwrap_or_else(|p| p.into_inner());
        while pending.is_none() {
            pending = self.wake.wait(pending).unwrap_or_else(|p| p.into_inner());
        }
        pending.take().expect("Notified display request")
    }
}
#[derive(Default)]
pub struct Display {
    worker: Option<Worker>,
    last: Option<(String, bool, u64)>,
    status: String,
}
impl Display {
    pub fn status(&self) -> &str {
        if self.status.is_empty() {
            "desactivada"
        } else {
            &self.status
        }
    }
    pub fn update(&mut self, hwnd: usize, enabled: bool, fps: f64) {
        if let Some(w) = &self.worker {
            while let Ok(code) = w.reply.try_recv() {
                self.status = match code {
                    0 => "modo conservado / restaurado",
                    1 => "sin frecuencia compatible a resolución nativa",
                    2 => "frecuencia solicitada al controlador; comprobar Hz reales",
                    3 => "cambio reservado por otra instancia o acceso no disponible",
                    _ => "el controlador rechazó el cambio",
                }
                .into();
            }
        }
        if !enabled && self.worker.is_none() {
            return;
        }
        let key = (
            identity(hwnd),
            enabled,
            if fps.is_finite() {
                (fps * 1000.0).round().max(0.0) as u64
            } else {
                0
            },
        );
        if self.last.as_ref() == Some(&key) {
            return;
        }
        if self.worker.is_none() {
            let pending = Arc::new(Mailbox::default());
            let requests = Arc::clone(&pending);
            let (reply_tx, reply) = mpsc::channel();
            let worker = thread::Builder::new()
                .name("display-refresh".into())
                .spawn(move || {
                    let context = unsafe { cine_display_create() };
                    if context.is_null() {
                        let _ = reply_tx.send(-1);
                        return;
                    }
                    while let Request::Mode(hwnd, on, fps) = requests.next() {
                        let code =
                            unsafe { cine_display_update(context, hwnd, i32::from(on), fps) };
                        let _ = reply_tx.send(code);
                    }
                    unsafe { cine_display_destroy(context) };
                });
            match worker {
                Ok(thread) => {
                    self.worker = Some(Worker {
                        pending,
                        thread: Some(thread),
                        reply,
                    })
                }
                Err(e) => {
                    self.status = e.to_string();
                    return;
                }
            }
        }
        if let Some(w) = &self.worker {
            w.pending.post(Request::Mode(hwnd, enabled, fps));
            self.last = Some(key);
        }
    }
}
impl Drop for Display {
    fn drop(&mut self) {
        if let Some(mut w) = self.worker.take() {
            w.pending.post(Request::Stop);
            if let Some(t) = w.thread.take() {
                let _ = t.join();
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn worker_shutdown_handles_queued_requests_without_a_live_monitor() {
        let mailbox = Arc::new(Mailbox::default());
        let barrier = Arc::new(std::sync::Barrier::new(2));
        let incoming = Arc::clone(&mailbox);
        let gate = Arc::clone(&barrier);
        let consumer = thread::spawn(move || {
            gate.wait();
            incoming.next()
        });
        mailbox.post(Request::Mode(1, true, 24.0));
        mailbox.post(Request::Mode(1, true, 25.0));
        mailbox.post(Request::Mode(1, false, 0.0));
        barrier.wait();
        assert_eq!(
            consumer.join().unwrap(),
            Request::Mode(1, false, 0.0),
            "A busy worker must consume the latest restore request, not an older enable"
        );
        let start = std::time::Instant::now();
        for _ in 0..32 {
            let mut display = Display::default();
            display.update(0, true, 24.0);
            display.update(0, false, 0.0);
            drop(display); // joins and releases the native context on its owner thread
        }
        assert!(start.elapsed() < std::time::Duration::from_secs(5));
    }
}
