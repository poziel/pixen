use anyhow::{Context, Result};
use std::process::{Child, Command};

/// Own the process tree so npm wrappers and MCP proxy children cannot outlive AI.
pub(super) struct Process(pub Child, #[cfg(windows)] Job);
impl Process {
    pub fn spawn(command: &mut Command) -> Result<Self> {
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // Assign the job before any wrapper can spawn an untracked child.
            command.creation_flags(0x08000000 | 0x00000004);
        }
        let mut child = command.spawn().context("Could not start Codex. Install the Codex CLI, run codex login in a terminal, then retry.")?;
        #[cfg(windows)]
        {
            match Job::attach(&child) {
                Ok(job) => {
                    let process = Self(child, job);
                    resume_primary_thread(&process.0)?;
                    Ok(process)
                }
                Err(error) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    Err(error)
                }
            }
        }
        #[cfg(not(windows))]
        {
            Ok(Self(child))
        }
    }
}

#[cfg(windows)]
fn resume_primary_thread(child: &Child) -> Result<()> {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, INVALID_HANDLE_VALUE},
        System::{Diagnostics::ToolHelp::*, Threading::*},
    };
    // Child was created suspended and has exactly its primary thread. Enumerating
    // obtains that thread handle without reaching into std::process internals.
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) };
    anyhow::ensure!(
        snapshot != INVALID_HANDLE_VALUE,
        "Find Codex primary thread: {}",
        std::io::Error::last_os_error()
    );
    let mut entry = THREADENTRY32 {
        dwSize: std::mem::size_of::<THREADENTRY32>() as u32,
        ..Default::default()
    };
    let mut found = None;
    // snapshot is a valid owned handle; entry has the required size field.
    let mut more = unsafe { Thread32First(snapshot, &mut entry) };
    while more != 0 {
        if entry.th32OwnerProcessID == child.id() {
            found = Some(entry.th32ThreadID);
            break;
        }
        more = unsafe { Thread32Next(snapshot, &mut entry) };
    }
    unsafe {
        CloseHandle(snapshot);
    }
    let thread_id = found.context("Codex primary thread disappeared before startup")?;
    let thread = unsafe { OpenThread(THREAD_SUSPEND_RESUME, 0, thread_id) };
    anyhow::ensure!(
        !thread.is_null(),
        "Open Codex primary thread: {}",
        std::io::Error::last_os_error()
    );
    // Resume only our newly created, job-assigned child's primary thread.
    let result = unsafe { ResumeThread(thread) };
    let error = std::io::Error::last_os_error();
    unsafe {
        CloseHandle(thread);
    }
    anyhow::ensure!(result != u32::MAX, "Resume Codex: {error}");
    Ok(())
}
impl Drop for Process {
    fn drop(&mut self) {
        drop(self.0.stdin.take());
        #[cfg(windows)]
        self.1.terminate();
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

pub(super) fn codex_command() -> Command {
    #[cfg(windows)]
    {
        // npm commonly installs a .cmd shim. No user prompt enters shell arguments.
        let paths: Vec<_> = std::env::var_os("PATH")
            .map(|p| std::env::split_paths(&p).collect())
            .unwrap_or_default();
        for directory in &paths {
            let executable = directory.join("codex.exe");
            if executable.is_file() {
                return Command::new(executable);
            }
        }
        for directory in paths {
            let shim = directory.join("codex.cmd");
            if shim.is_file() {
                let mut command = Command::new("cmd.exe");
                command.args(["/d", "/s", "/c"]);
                command.arg(format!(
                    "\"\"{}\" app-server --listen stdio://\"",
                    shim.display()
                ));
                return command;
            }
        }
    }
    Command::new("codex")
}

pub(super) fn app_server_command() -> Command {
    let mut command = codex_command();
    // The npm shim command already contains the fixed app-server arguments.
    #[cfg(windows)]
    if command
        .get_program()
        .to_string_lossy()
        .eq_ignore_ascii_case("cmd.exe")
    {
        return command;
    }
    command.args(["app-server", "--listen", "stdio://"]);
    command
}

#[cfg(windows)]
struct Job(windows_sys::Win32::Foundation::HANDLE);
#[cfg(windows)]
impl Job {
    fn attach(child: &Child) -> Result<Self> {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::System::JobObjects::*;
        // Unnamed job, non-inheritable handle. The worker is its sole owner.
        let handle = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        anyhow::ensure!(
            !handle.is_null(),
            "Create Codex process job: {}",
            std::io::Error::last_os_error()
        );
        let job = Self(handle);
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        // limits points to a live structure of exactly the supplied size.
        let configured = unsafe {
            SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                std::mem::size_of_val(&limits) as u32,
            )
        };
        anyhow::ensure!(
            configured != 0,
            "Configure Codex process job: {}",
            std::io::Error::last_os_error()
        );
        // The child handle remains valid while this worker owns Child.
        let assigned = unsafe { AssignProcessToJobObject(handle, child.as_raw_handle().cast()) };
        anyhow::ensure!(
            assigned != 0,
            "Track Codex process tree: {}",
            std::io::Error::last_os_error()
        );
        Ok(job)
    }
    fn terminate(&self) {
        // Only processes assigned to this private job are affected.
        unsafe {
            windows_sys::Win32::System::JobObjects::TerminateJobObject(self.0, 1);
        }
    }
}
#[cfg(windows)]
impl Drop for Job {
    fn drop(&mut self) {
        // Release the unique owned handle; kill-on-close also covers unwinding.
        unsafe {
            windows_sys::Win32::Foundation::CloseHandle(self.0);
        }
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use std::{
        process::Stdio,
        time::{Duration, Instant},
    };
    use windows_sys::Win32::{
        Foundation::{CloseHandle, WAIT_OBJECT_0},
        System::{JobObjects::*, Threading::*},
    };

    #[test]
    fn dropping_agent_process_terminates_wrapper_and_descendant() {
        let mut command = Command::new("cmd.exe");
        command
            .args(["/d", "/c", "ping.exe -t 127.0.0.1"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let process = Process::spawn(&mut command).unwrap();
        #[repr(C)]
        struct ProcessIds {
            assigned: u32,
            count: u32,
            ids: [usize; 16],
        }
        let until = Instant::now() + Duration::from_secs(3);
        let handles = loop {
            let mut ids = ProcessIds {
                assigned: 0,
                count: 0,
                ids: [0; 16],
            };
            // Query our own private job into a buffer large enough for this fixture.
            let ok = unsafe {
                QueryInformationJobObject(
                    process.1.0,
                    JobObjectBasicProcessIdList,
                    (&mut ids as *mut ProcessIds).cast(),
                    std::mem::size_of_val(&ids) as u32,
                    std::ptr::null_mut(),
                )
            };
            assert_ne!(ok, 0);
            if ids.count >= 2 {
                let mut handles = Vec::new();
                for id in &ids.ids[..ids.count as usize] {
                    let handle = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, *id as u32) };
                    assert!(!handle.is_null());
                    handles.push(handle);
                }
                break handles;
            }
            assert!(Instant::now() < until, "wrapper did not spawn its child");
            std::thread::sleep(Duration::from_millis(10));
        };
        drop(process);
        for handle in handles {
            let ended = unsafe { WaitForSingleObject(handle, 2000) };
            unsafe {
                CloseHandle(handle);
            }
            assert_eq!(ended, WAIT_OBJECT_0, "an owned child survived shutdown");
        }
    }
}
