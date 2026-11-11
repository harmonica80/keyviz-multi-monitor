//! Shared "wake on demand" plumbing for the native overlay threads.
//!
//! Every native overlay (key display, cursor highlight, drawing canvas) owns a
//! Win32 window on its own thread and receives work through an `mpsc` channel.
//! Previously each of those threads woke up every 16 ms to poll the channel
//! and pump messages, which cost ~250 wake-ups per second while the app was
//! completely idle.
//!
//! The pattern here is:
//!
//! * The thread blocks in `MsgWaitForMultipleObjectsEx` until either a window
//!   message arrives or the (generous) safety timeout elapses.
//! * Senders wrap their channel in [`WakingSender`]. After pushing a command
//!   they post a `WM_NULL` thread message, which wakes the blocked wait
//!   immediately. If the queue is not ready yet (thread still starting) the
//!   post simply fails and the safety timeout picks the command up.
//!
//! The result is zero wake-ups while idle, and sub-millisecond latency when a
//! command is queued.

#[cfg(target_os = "windows")]
mod platform {
    use std::sync::{
        atomic::{AtomicU32, Ordering},
        mpsc::{SendError, Sender},
        Arc,
    };

    use windows::Win32::{
        Foundation::{HWND, LPARAM, WPARAM},
        System::Threading::GetCurrentThreadId,
        UI::WindowsAndMessaging::{
            MsgWaitForMultipleObjectsEx, PeekMessageW, PostThreadMessageW, MSG, MWMO_INPUTAVAILABLE,
            PM_NOREMOVE, QS_ALLINPUT, WM_NULL, WM_USER,
        },
    };

    /// Upper bound on how long a native thread sleeps without being woken.
    /// Only matters if a `PostThreadMessageW` wake-up is lost (thread not yet
    /// initialised); normal operation never waits this long.
    pub const SAFETY_TIMEOUT_MS: u32 = 1000;

    /// Shared handle that lets any thread wake a native overlay thread.
    #[derive(Clone, Default)]
    pub struct WakeHandle {
        thread_id: Arc<AtomicU32>,
    }

    impl WakeHandle {
        pub fn new() -> Self {
            Self::default()
        }

        /// Must be called once from the native thread, before it enters its
        /// message loop. Creates the thread message queue so that
        /// `PostThreadMessageW` from other threads succeeds.
        pub fn register_current_thread(&self) {
            unsafe {
                let mut message = MSG::default();
                // Forces creation of the message queue for this thread.
                let _ = PeekMessageW(&mut message, HWND(0), WM_USER, WM_USER, PM_NOREMOVE);
                self.thread_id.store(GetCurrentThreadId(), Ordering::Release);
            }
        }

        pub fn unregister(&self) {
            self.thread_id.store(0, Ordering::Release);
        }

        /// Wake the native thread if it is blocked waiting for input.
        pub fn wake(&self) {
            let thread_id = self.thread_id.load(Ordering::Acquire);
            if thread_id == 0 {
                return;
            }
            unsafe {
                let _ = PostThreadMessageW(thread_id, WM_NULL, WPARAM(0), LPARAM(0));
            }
        }
    }

    /// `mpsc::Sender` that also wakes the receiving native thread.
    ///
    /// It deliberately exposes the same `send` signature as `Sender`, so the
    /// existing call sites keep working unchanged.
    pub struct WakingSender<T> {
        sender: Sender<T>,
        wake: WakeHandle,
    }

    // Manual impl: `derive(Clone)` would wrongly require `T: Clone`.
    impl<T> Clone for WakingSender<T> {
        fn clone(&self) -> Self {
            Self {
                sender: self.sender.clone(),
                wake: self.wake.clone(),
            }
        }
    }

    impl<T> WakingSender<T> {
        pub fn new(sender: Sender<T>, wake: WakeHandle) -> Self {
            Self { sender, wake }
        }

        pub fn send(&self, value: T) -> Result<(), SendError<T>> {
            let result = self.sender.send(value);
            if result.is_ok() {
                self.wake.wake();
            }
            result
        }
    }

    /// Block until a window/thread message is available or the timeout
    /// elapses. Returns immediately if messages are already pending.
    pub fn wait_for_input(timeout_ms: u32) {
        unsafe {
            let _ = MsgWaitForMultipleObjectsEx(None, timeout_ms, QS_ALLINPUT, MWMO_INPUTAVAILABLE);
        }
    }
}

#[cfg(target_os = "windows")]
pub use platform::{wait_for_input, WakeHandle, WakingSender, SAFETY_TIMEOUT_MS};
