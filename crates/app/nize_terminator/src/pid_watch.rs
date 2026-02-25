// @awa-component: PLAN-005-PidWatch
//! Platform-specific parent-death detection.
//!
//! - macOS: `kqueue` with `EVFILT_PROC` + `NOTE_EXIT` (instant notification).
//! - Linux: `pidfd_open` + `poll` (instant notification, kernel ≥5.3).
//! - Windows: `OpenProcess` + `WaitForSingleObject` (instant notification).
//! - Fallback (Unix): `kill(pid, 0)` polling every 1 second.

/// Block until the given PID exits.
///
/// Uses the most efficient platform-specific mechanism available.
pub fn wait_for_pid_exit(pid: u32) {
    #[cfg(unix)]
    {
        if !is_pid_alive_unix(pid) {
            return;
        }
    }

    #[cfg(target_os = "macos")]
    {
        if kqueue_wait(pid) {
            return;
        }
    }

    #[cfg(target_os = "linux")]
    {
        if pidfd_wait(pid) {
            return;
        }
    }

    // @awa-impl: PLAN-006-3.1
    #[cfg(target_os = "windows")]
    {
        windows_wait(pid);
        return;
    }

    // Fallback: poll with kill(pid, 0)
    #[cfg(unix)]
    poll_wait(pid);
}

/// Check whether a PID is still alive via `kill(pid, 0)`.
#[cfg(unix)]
fn is_pid_alive_unix(pid: u32) -> bool {
    // SAFETY: kill(pid, 0) sends no signal — only checks existence.
    unsafe { libc::kill(pid as libc::pid_t, 0) == 0 }
}

/// Fallback: poll `kill(pid, 0)` every second until the process exits.
#[cfg(unix)]
fn poll_wait(pid: u32) {
    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
        if !is_pid_alive_unix(pid) {
            return;
        }
    }
}

/// macOS: use kqueue EVFILT_PROC + NOTE_EXIT for instant notification.
/// Returns `true` if kqueue succeeded, `false` if caller should use fallback.
#[cfg(target_os = "macos")]
fn kqueue_wait(pid: u32) -> bool {
    // SAFETY: kqueue/kevent are standard macOS syscalls.
    unsafe {
        let kq = libc::kqueue();
        if kq < 0 {
            return false;
        }

        let change = libc::kevent {
            ident: pid as libc::uintptr_t,
            filter: libc::EVFILT_PROC,
            flags: libc::EV_ADD | libc::EV_ONESHOT,
            fflags: libc::NOTE_EXIT,
            data: 0,
            udata: std::ptr::null_mut(),
        };

        let mut event: libc::kevent = std::mem::zeroed();

        // Register and wait in one call.
        let n = libc::kevent(
            kq,
            &change as *const libc::kevent,
            1,
            &mut event as *mut libc::kevent,
            1,
            std::ptr::null(), // no timeout — block indefinitely
        );

        libc::close(kq);

        // If the process already exited between our alive-check and kevent,
        // kevent returns immediately with the exit event or an error.
        // Either way, the parent is gone.
        if n < 0 {
            // kevent failed — might be ESRCH (process already gone). Verify.
            return !is_pid_alive_unix(pid);
        }

        true
    }
}

/// Linux: use pidfd_open + poll for instant notification.
/// Returns `true` if pidfd succeeded, `false` if caller should use fallback.
#[cfg(target_os = "linux")]
fn pidfd_wait(pid: u32) -> bool {
    // SAFETY: pidfd_open and poll are standard Linux syscalls.
    unsafe {
        let fd = libc::syscall(libc::SYS_pidfd_open, pid as libc::pid_t, 0_i32) as libc::c_int;
        if fd < 0 {
            return false;
        }

        let mut pfd = libc::pollfd {
            fd,
            events: libc::POLLIN,
            revents: 0,
        };

        // Block indefinitely until the process exits.
        libc::poll(&mut pfd as *mut libc::pollfd, 1, -1);

        libc::close(fd);
        true
    }
}

// @awa-impl: PLAN-006-3.1
/// Windows: use `OpenProcess(SYNCHRONIZE)` + `WaitForSingleObject(INFINITE)`.
///
/// Opens the target process with SYNCHRONIZE access (minimal privilege),
/// then blocks until the process terminates.
#[cfg(target_os = "windows")]
fn windows_wait(pid: u32) {
    use windows_sys::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0};
    use windows_sys::Win32::System::Threading::{
        INFINITE, OpenProcess, SYNCHRONIZE, WaitForSingleObject,
    };

    // SAFETY: OpenProcess + WaitForSingleObject are standard Win32 APIs.
    unsafe {
        let handle = OpenProcess(SYNCHRONIZE, 0, pid);
        if handle.is_null() {
            // Process doesn't exist or we lack permission — already dead.
            return;
        }

        // Block until the process exits.
        let result = WaitForSingleObject(handle, INFINITE);
        CloseHandle(handle);

        if result != WAIT_OBJECT_0 {
            // Fallback: poll with GetExitCodeProcess if wait failed.
            windows_poll_wait(pid);
        }
    }
}

// @awa-impl: PLAN-006-3.1
/// Windows fallback: poll `OpenProcess` every second (for permission-denied edge cases).
#[cfg(target_os = "windows")]
fn windows_poll_wait(pid: u32) {
    use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};

    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
        // SAFETY: OpenProcess with minimal access just to check existence.
        let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
        if handle.is_null() {
            return; // Process no longer exists.
        }
        unsafe { windows_sys::Win32::Foundation::CloseHandle(handle) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // @awa-test: PLAN-005-PidWatch
    #[cfg(unix)]
    #[test]
    fn wait_for_already_dead_pid() {
        // Spawn a process and immediately kill it, then verify wait_for_pid_exit returns.
        let child = std::process::Command::new("sleep")
            .arg("60")
            .spawn()
            .expect("spawn sleep");

        let pid = child.id();

        // Kill the child
        // SAFETY: sending SIGKILL to a known child process.
        unsafe {
            libc::kill(pid as libc::pid_t, libc::SIGKILL);
        }

        // Reap the zombie so the OS recycles the PID.
        let mut child = child;
        let _ = child.wait();

        // Now wait_for_pid_exit should return immediately.
        wait_for_pid_exit(pid);
    }

    // @awa-test: PLAN-005-PidWatch
    #[cfg(unix)]
    #[test]
    fn wait_detects_exit() {
        let child = std::process::Command::new("sleep")
            .arg("60")
            .spawn()
            .expect("spawn sleep");

        let pid = child.id();

        // Kill the child from another thread after a short delay.
        let handle = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(200));
            // SAFETY: sending SIGTERM to a known child process.
            unsafe {
                libc::kill(pid as libc::pid_t, libc::SIGTERM);
            }
        });

        // Reap the zombie in the background so wait_for_pid_exit can detect death.
        let mut child = child;
        let reaper = std::thread::spawn(move || {
            let _ = child.wait();
        });

        wait_for_pid_exit(pid);

        handle.join().expect("killer thread");
        reaper.join().expect("reaper thread");
    }

    #[cfg(unix)]
    #[test]
    fn is_pid_alive_returns_false_for_nonexistent() {
        // PID 0 is the kernel — we can't signal it. Use a very high PID.
        assert!(!is_pid_alive_unix(4_000_000));
    }

    #[cfg(unix)]
    #[test]
    fn is_pid_alive_returns_true_for_self() {
        let pid = std::process::id();
        assert!(is_pid_alive_unix(pid));
    }

    // @awa-test: PLAN-006-3.5
    #[cfg(windows)]
    #[test]
    fn wait_for_already_dead_pid_windows() {
        let child = std::process::Command::new("cmd")
            .args(["/C", "timeout /t 60"])
            .spawn()
            .expect("spawn timeout");

        let pid = child.id();

        // Kill the child.
        let mut child = child;
        let _ = child.kill();
        let _ = child.wait();

        // Now wait_for_pid_exit should return immediately.
        wait_for_pid_exit(pid);
    }

    // @awa-test: PLAN-006-3.5
    #[cfg(windows)]
    #[test]
    fn wait_detects_exit_windows() {
        let child = std::process::Command::new("cmd")
            .args(["/C", "timeout /t 60"])
            .spawn()
            .expect("spawn timeout");

        let pid = child.id();

        let mut child_for_kill = child;
        let handle = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(200));
            let _ = child_for_kill.kill();
            let _ = child_for_kill.wait();
        });

        wait_for_pid_exit(pid);

        handle.join().expect("killer thread");
    }
}
