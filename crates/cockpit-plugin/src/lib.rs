use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

#[cfg(unix)]
use std::os::unix::process::CommandExt;

use cockpit_world::{StatePatch, StatePatchTarget, WorldSnapshot};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const PLUGIN_API_VERSION: u32 = 1;

#[cfg(windows)]
type PluginJobHandle = windows_sys::Win32::Foundation::HANDLE;
#[cfg(not(windows))]
type PluginJobHandle = ();

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PluginPermission {
    WorldRead,
    WorldWrite,
    Network,
    FilesystemRead,
    ChildProcess,
    Threads,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PluginFailurePolicy {
    DisablePlugin,
    PauseRun,
    FailRun,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginManifest {
    pub id: String,
    pub version: String,
    pub api_contract: u32,
    pub permissions: Vec<PluginPermission>,
    pub schema: Value,
    pub hash: String,
    #[serde(default)]
    pub signature: Option<String>,
    /// Program and arguments for the production out-of-process executor.
    #[serde(default)]
    pub command: Option<Vec<String>>,
    /// Absolute paths the macOS process sandbox may read. Empty by default;
    /// an empty list is omitted from canonical hashing for old manifests.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub filesystem_read_paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StateDiff {
    pub plugin_id: String,
    pub patch: StatePatch,
    pub expected_state_version: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginStatus {
    Discovered,
    Ready,
    Disabled,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginFailure {
    pub plugin_id: String,
    pub version: String,
    pub reason: String,
    pub decision: PluginFailurePolicy,
    #[serde(default)]
    pub execution: Option<PluginExecutionEvidence>,
}

/// Bounded process facts for a plugin tick. It intentionally excludes plugin
/// output, command arguments, and environment data from recordings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginExecutionEvidence {
    pub elapsed_ms: u64,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub terminated_process_group: bool,
    pub stdout_bytes: usize,
    pub stderr_bytes: usize,
}

pub trait PluginExecutor: Send {
    fn tick(&mut self, snapshot: &WorldSnapshot) -> Result<Vec<StateDiff>, String>;

    fn take_execution_evidence(&mut self) -> Option<PluginExecutionEvidence> {
        None
    }
}

/// Executes an untrusted plugin as a fresh child process for every tick.
/// The process receives one JSON `WorldSnapshot` on stdin and must emit one
/// JSON `Vec<StateDiff>` on stdout. A fresh process prevents state and file
/// descriptor reuse across ticks; more importantly, the host can kill a
/// process that exceeds the real wall-clock deadline.
pub struct ProcessPluginExecutor {
    program: String,
    arguments: Vec<String>,
    permissions: BTreeSet<PluginPermission>,
    filesystem_read_paths: Vec<PathBuf>,
    deadline: Duration,
    max_output_bytes: usize,
    last_execution: Option<PluginExecutionEvidence>,
}

impl ProcessPluginExecutor {
    pub const DEFAULT_MAX_OUTPUT_BYTES: usize = 1_048_576;

    pub fn new(program: impl Into<String>, arguments: Vec<String>, deadline: Duration) -> Self {
        Self {
            program: program.into(),
            arguments,
            permissions: BTreeSet::new(),
            filesystem_read_paths: Vec::new(),
            deadline,
            max_output_bytes: Self::DEFAULT_MAX_OUTPUT_BYTES,
            last_execution: None,
        }
    }

    pub fn from_command(command: Vec<String>, deadline: Duration) -> Result<Self, String> {
        let (program, arguments) = command
            .split_first()
            .ok_or_else(|| "plugin command must include a program".to_string())?;
        Ok(Self::new(program.clone(), arguments.to_vec(), deadline))
    }

    /// Bind OS-enforced sandbox permissions to an already validated manifest.
    /// Callers that construct an executor directly receive the restrictive
    /// default (no network or child processes).
    pub fn with_permissions(
        mut self,
        permissions: impl IntoIterator<Item = PluginPermission>,
    ) -> Self {
        self.permissions = permissions.into_iter().collect();
        self
    }

    pub fn from_command_with_permissions(
        command: Vec<String>,
        deadline: Duration,
        permissions: impl IntoIterator<Item = PluginPermission>,
    ) -> Result<Self, String> {
        Ok(Self::from_command(command, deadline)?.with_permissions(permissions))
    }

    pub fn with_filesystem_read_paths(
        mut self,
        paths: impl IntoIterator<Item = impl Into<PathBuf>>,
    ) -> Self {
        self.filesystem_read_paths = paths.into_iter().map(Into::into).collect();
        self
    }

    pub fn from_command_with_permissions_and_read_paths(
        command: Vec<String>,
        deadline: Duration,
        permissions: impl IntoIterator<Item = PluginPermission>,
        filesystem_read_paths: impl IntoIterator<Item = impl Into<PathBuf>>,
    ) -> Result<Self, String> {
        Ok(Self::from_command(command, deadline)?
            .with_permissions(permissions)
            .with_filesystem_read_paths(filesystem_read_paths))
    }

    pub fn with_max_output_bytes(mut self, max_output_bytes: usize) -> Self {
        self.max_output_bytes = max_output_bytes;
        self
    }
}

impl PluginExecutor for ProcessPluginExecutor {
    fn tick(&mut self, snapshot: &WorldSnapshot) -> Result<Vec<StateDiff>, String> {
        self.last_execution = None;
        let started = Instant::now();
        let mut command = plugin_command(
            &self.program,
            &self.arguments,
            &self.permissions,
            &self.filesystem_read_paths,
        );
        command
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(unix)]
        // The child is isolated into a process group before exec, so a
        // deadline kills descendants that inherited its group as well.
        unsafe {
            #[cfg(target_os = "linux")]
            let sandbox_permissions = self.permissions.clone();
            #[cfg(target_os = "linux")]
            let sandbox_read_paths = self.filesystem_read_paths.clone();
            command.pre_exec(move || {
                if libc::setpgid(0, 0) == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                #[cfg(target_os = "linux")]
                apply_linux_filesystem_sandbox(&sandbox_read_paths)?;
                #[cfg(target_os = "linux")]
                apply_linux_sandbox(&sandbox_permissions)?;
                Ok(())
            });
        }
        let mut child = command
            .spawn()
            .map_err(|error| format!("plugin process spawn failed: {error}"))?;
        let pid = child.id();
        let job_handle = match create_windows_job(&child) {
            Ok(handle) => handle,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("plugin Windows Job Object setup failed: {error}"));
            }
        };
        let input = serde_json::to_vec(snapshot)
            .map_err(|error| format!("plugin snapshot serialization failed: {error}"))?;
        let Some(mut stdin) = child.stdin.take() else {
            let _ = terminate_plugin_process(&mut child, pid, job_handle);
            self.last_execution = Some(execution_evidence(started, None, false, true, 0, 0));
            return Err("plugin process stdin was unavailable".to_string());
        };
        let (stdin_result_tx, stdin_result_rx) = std::sync::mpsc::sync_channel(1);
        let stdin_writer = std::thread::spawn(move || {
            let result = stdin.write_all(&input);
            let _ = stdin_result_tx.send(result);
        });

        // Keep every blocking pipe operation off the coordinator thread. A
        // malicious plugin can stop reading stdin just as easily as it can
        // fill stdout/stderr; the deadline must still reach the process group.
        let stdout = child.stdout.take().expect("stdout configured as piped");
        let stderr = child.stderr.take().expect("stderr configured as piped");
        let max_output_bytes = self.max_output_bytes;
        let stdout_reader = std::thread::spawn(move || read_limited(stdout, max_output_bytes));
        let stderr_reader = std::thread::spawn(move || read_limited(stderr, max_output_bytes));
        let mut stdin_complete = false;

        let status = loop {
            if !stdin_complete {
                match stdin_result_rx.try_recv() {
                    Ok(Ok(())) => stdin_complete = true,
                    Ok(Err(error)) => {
                        let _ = terminate_plugin_process(&mut child, pid, job_handle);
                        let _ = stdin_writer.join();
                        let _ = stdout_reader.join();
                        let _ = stderr_reader.join();
                        self.last_execution =
                            Some(execution_evidence(started, None, false, true, 0, 0));
                        return Err(format!("plugin process stdin failed: {error}"));
                    }
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        let _ = terminate_plugin_process(&mut child, pid, job_handle);
                        let _ = stdin_writer.join();
                        let _ = stdout_reader.join();
                        let _ = stderr_reader.join();
                        self.last_execution =
                            Some(execution_evidence(started, None, false, true, 0, 0));
                        return Err("plugin process stdin writer panicked".to_string());
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => {}
                }
            }
            match child.try_wait() {
                Err(error) => {
                    let _ = terminate_plugin_process(&mut child, pid, job_handle);
                    let _ = stdin_writer.join();
                    let _ = stdout_reader.join();
                    let _ = stderr_reader.join();
                    self.last_execution =
                        Some(execution_evidence(started, None, false, true, 0, 0));
                    return Err(format!("plugin process status failed: {error}"));
                }
                Ok(Some(status)) => break status,
                Ok(None) if started.elapsed() >= self.deadline => {
                    let status = terminate_plugin_process(&mut child, pid, job_handle).ok();
                    let _ = stdin_writer.join();
                    let _ = stdout_reader.join();
                    let _ = stderr_reader.join();
                    self.last_execution =
                        Some(execution_evidence(started, status, true, true, 0, 0));
                    return Err(format!(
                        "plugin process exceeded {}ms deadline and was terminated",
                        self.deadline.as_millis()
                    ));
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(1)),
            }
        };
        if !stdin_complete {
            match stdin_result_rx.recv() {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    let _ = stdin_writer.join();
                    let _ = stdout_reader.join();
                    let _ = stderr_reader.join();
                    close_windows_job(job_handle);
                    self.last_execution = Some(execution_evidence(
                        started,
                        Some(status),
                        false,
                        false,
                        0,
                        0,
                    ));
                    return Err(format!("plugin process stdin failed: {error}"));
                }
                Err(_) => {
                    let _ = stdin_writer.join();
                    let _ = stdout_reader.join();
                    let _ = stderr_reader.join();
                    self.last_execution = Some(execution_evidence(
                        started,
                        Some(status),
                        false,
                        false,
                        0,
                        0,
                    ));
                    return Err("plugin process stdin writer panicked".to_string());
                }
            }
        }
        let _ = stdin_writer.join();
        let stdout = stdout_reader
            .join()
            .map_err(|_| "plugin stdout reader panicked".to_string())?
            .map_err(|error| format!("plugin stdout read failed: {error}"))?;
        let stderr = stderr_reader
            .join()
            .map_err(|_| "plugin stderr reader panicked".to_string())?
            .map_err(|error| format!("plugin stderr read failed: {error}"))?;
        self.last_execution = Some(execution_evidence(
            started,
            Some(status),
            false,
            false,
            stdout.len(),
            stderr.len(),
        ));
        if !status.success() {
            return Err(format!(
                "plugin process exited with {status}: {}",
                String::from_utf8_lossy(&stderr)
            ));
        }
        serde_json::from_slice(&stdout)
            .map_err(|error| format!("plugin process returned invalid StateDiff JSON: {error}"))
    }

    fn take_execution_evidence(&mut self) -> Option<PluginExecutionEvidence> {
        self.last_execution.take()
    }
}

fn plugin_command(
    program: &str,
    arguments: &[String],
    permissions: &BTreeSet<PluginPermission>,
    filesystem_read_paths: &[PathBuf],
) -> Command {
    #[cfg(target_os = "macos")]
    {
        let mut command = Command::new("sandbox-exec");
        command
            .arg("-p")
            .arg(macos_sandbox_profile(
                program,
                permissions,
                filesystem_read_paths,
            ))
            .arg(program)
            .args(arguments);
        command
    }
    #[cfg(not(target_os = "macos"))]
    {
        let mut command = Command::new(program);
        command.args(arguments);
        let _ = (permissions, filesystem_read_paths);
        command
    }
}

#[cfg(target_os = "linux")]
fn apply_linux_filesystem_sandbox(read_paths: &[PathBuf]) -> Result<(), std::io::Error> {
    use std::{ffi::CString, os::unix::ffi::OsStrExt};

    // Landlock is unprivileged and inherited across exec, making it suitable
    // for a plugin child without requiring a mount namespace or CAP_SYS_ADMIN.
    // The runtime directories are the minimum read/execute surface needed by
    // dynamically linked binaries; manifest paths extend that surface only
    // when the plugin explicitly declares FilesystemRead.
    const LANDLOCK_CREATE_RULESET: libc::c_long = 444;
    const LANDLOCK_ADD_RULE: libc::c_long = 445;
    const LANDLOCK_RESTRICT_SELF: libc::c_long = 446;
    const LANDLOCK_CREATE_RULESET_VERSION: libc::c_long = 1;
    const LANDLOCK_RULE_TYPE_PATH_BENEATH: u32 = 1;
    const LANDLOCK_ACCESS_FS_EXECUTE: u64 = 1 << 0;
    const LANDLOCK_ACCESS_FS_READ_FILE: u64 = 1 << 2;
    const LANDLOCK_ACCESS_FS_READ_DIR: u64 = 1 << 3;
    const LANDLOCK_ACCESS_FS_WRITE_FILE: u64 = 1 << 1;
    const LANDLOCK_ACCESS_FS_REMOVE_DIR: u64 = 1 << 4;
    const LANDLOCK_ACCESS_FS_REMOVE_FILE: u64 = 1 << 5;
    const LANDLOCK_ACCESS_FS_MAKE_CHAR: u64 = 1 << 6;
    const LANDLOCK_ACCESS_FS_MAKE_DIR: u64 = 1 << 7;
    const LANDLOCK_ACCESS_FS_MAKE_REG: u64 = 1 << 8;
    const LANDLOCK_ACCESS_FS_MAKE_SOCK: u64 = 1 << 9;
    const LANDLOCK_ACCESS_FS_MAKE_FIFO: u64 = 1 << 10;
    const LANDLOCK_ACCESS_FS_MAKE_BLOCK: u64 = 1 << 11;
    const LANDLOCK_ACCESS_FS_MAKE_SYM: u64 = 1 << 12;
    const LANDLOCK_ACCESS_FS_REFER: u64 = 1 << 13;
    const LANDLOCK_ACCESS_FS_TRUNCATE: u64 = 1 << 14;
    const READ_EXECUTE: u64 =
        LANDLOCK_ACCESS_FS_EXECUTE | LANDLOCK_ACCESS_FS_READ_FILE | LANDLOCK_ACCESS_FS_READ_DIR;
    const BASE_FS: u64 = READ_EXECUTE
        | LANDLOCK_ACCESS_FS_WRITE_FILE
        | LANDLOCK_ACCESS_FS_REMOVE_DIR
        | LANDLOCK_ACCESS_FS_REMOVE_FILE
        | LANDLOCK_ACCESS_FS_MAKE_CHAR
        | LANDLOCK_ACCESS_FS_MAKE_DIR
        | LANDLOCK_ACCESS_FS_MAKE_REG
        | LANDLOCK_ACCESS_FS_MAKE_SOCK
        | LANDLOCK_ACCESS_FS_MAKE_FIFO
        | LANDLOCK_ACCESS_FS_MAKE_BLOCK
        | LANDLOCK_ACCESS_FS_MAKE_SYM;

    #[repr(C)]
    struct RulesetAttr {
        handled_access_fs: u64,
    }
    #[repr(C)]
    struct PathBeneath {
        parent_fd: u64,
        allowed_access: u64,
    }

    if unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    // Ask the kernel which Landlock ABI is available so newer handled rights
    // are used only when the running kernel understands them.
    let abi = unsafe {
        libc::syscall(
            LANDLOCK_CREATE_RULESET,
            std::ptr::null::<RulesetAttr>(),
            0usize,
            LANDLOCK_CREATE_RULESET_VERSION,
        )
    };
    if abi < 1 {
        return Err(std::io::Error::last_os_error());
    }
    let mut handled_access_fs = BASE_FS;
    if abi >= 2 {
        handled_access_fs |= LANDLOCK_ACCESS_FS_REFER;
    }
    if abi >= 3 {
        handled_access_fs |= LANDLOCK_ACCESS_FS_TRUNCATE;
    }
    let attr = RulesetAttr { handled_access_fs };
    // SAFETY: syscall arguments point to stable, repr(C) values for this call.
    let ruleset_fd = unsafe {
        libc::syscall(
            LANDLOCK_CREATE_RULESET,
            &attr as *const RulesetAttr,
            std::mem::size_of::<RulesetAttr>(),
            0u32,
        )
    };
    if ruleset_fd < 0 {
        return Err(std::io::Error::last_os_error());
    }

    let mut allowed = vec![
        PathBuf::from("/usr"),
        PathBuf::from("/bin"),
        PathBuf::from("/sbin"),
        PathBuf::from("/lib"),
        PathBuf::from("/lib64"),
        PathBuf::from("/dev"),
    ];
    allowed.extend(read_paths.iter().cloned());
    for path in allowed {
        let bytes = path.as_os_str().as_bytes();
        let path_c = CString::new(bytes).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "sandbox path contains NUL",
            )
        })?;
        // SAFETY: path_c is NUL-terminated and flags request an O_PATH handle.
        let parent_fd = unsafe { libc::open(path_c.as_ptr(), libc::O_PATH | libc::O_CLOEXEC) };
        if parent_fd < 0 {
            let error = std::io::Error::last_os_error();
            if matches!(
                error.raw_os_error(),
                Some(libc::ENOENT) | Some(libc::ENOTDIR)
            ) {
                continue;
            }
            unsafe { libc::close(ruleset_fd as libc::c_int) };
            return Err(error);
        }
        let rule = PathBeneath {
            parent_fd: parent_fd as u64,
            allowed_access: READ_EXECUTE,
        };
        // SAFETY: rule points to a valid parent fd and remains alive for call.
        let result = unsafe {
            libc::syscall(
                LANDLOCK_ADD_RULE,
                ruleset_fd,
                LANDLOCK_RULE_TYPE_PATH_BENEATH,
                &rule as *const PathBeneath,
                0u32,
            )
        };
        unsafe {
            libc::close(parent_fd);
        }
        if result < 0 {
            let error = std::io::Error::last_os_error();
            unsafe { libc::close(ruleset_fd as libc::c_int) };
            return Err(error);
        }
    }
    // SAFETY: the ruleset fd was created by this process and is now restricted.
    let result = unsafe { libc::syscall(LANDLOCK_RESTRICT_SELF, ruleset_fd, 0u32) };
    let error = if result < 0 {
        Some(std::io::Error::last_os_error())
    } else {
        None
    };
    unsafe {
        libc::close(ruleset_fd as libc::c_int);
    }
    error.map_or(Ok(()), Err)
}

#[cfg(target_os = "linux")]
fn apply_linux_sandbox(permissions: &BTreeSet<PluginPermission>) -> Result<(), std::io::Error> {
    let mut denied = Vec::new();
    if !permissions.contains(&PluginPermission::Network) {
        denied.extend([
            libc::SYS_socket,
            libc::SYS_socketpair,
            libc::SYS_connect,
            libc::SYS_bind,
            libc::SYS_listen,
            libc::SYS_accept,
            libc::SYS_accept4,
            libc::SYS_sendto,
            libc::SYS_sendmsg,
            libc::SYS_sendmmsg,
            libc::SYS_recvfrom,
            libc::SYS_recvmsg,
            libc::SYS_recvmmsg,
        ]);
    }
    if !permissions.contains(&PluginPermission::ChildProcess) {
        denied.extend([libc::SYS_clone, libc::SYS_fork, libc::SYS_vfork]);
        #[cfg(any(target_arch = "x86_64", target_arch = "aarch64", target_arch = "x86"))]
        denied.push(libc::SYS_clone3);
    }
    if denied.is_empty() {
        return Ok(());
    }

    // Validate the native syscall architecture before loading the filter. The
    // architecture constants are stable Linux UAPI values; refusing an
    // unknown architecture avoids silently installing a bypassable filter.
    #[cfg(target_arch = "x86_64")]
    const AUDIT_ARCH: u32 = 0xc000_003e;
    #[cfg(target_arch = "aarch64")]
    const AUDIT_ARCH: u32 = 0xc000_00b7;
    #[cfg(target_arch = "x86")]
    const AUDIT_ARCH: u32 = 0x4000_0003;
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64", target_arch = "x86")))]
    return Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "Linux plugin sandbox has no syscall architecture profile",
    ));

    const SECCOMP_RET_KILL_PROCESS: u32 = 0x8000_0000;
    const SECCOMP_RET_ERRNO: u32 = 0x0005_0000;
    const BPF_LD: u16 = 0x00;
    const BPF_W: u16 = 0x00;
    const BPF_ABS: u16 = 0x20;
    const BPF_JMP: u16 = 0x05;
    const BPF_JEQ: u16 = 0x10;
    const BPF_K: u16 = 0x00;
    const BPF_RET: u16 = 0x06;
    const BPF_STMT: u16 = BPF_LD | BPF_W | BPF_ABS;
    let mut filter = vec![
        libc::sock_filter {
            code: BPF_STMT,
            jt: 0,
            jf: 0,
            k: 4,
        },
        libc::sock_filter {
            code: BPF_JMP | BPF_JEQ | BPF_K,
            jt: 1,
            jf: 0,
            k: AUDIT_ARCH,
        },
        libc::sock_filter {
            code: BPF_RET | BPF_K,
            jt: 0,
            jf: 0,
            k: SECCOMP_RET_KILL_PROCESS,
        },
    ];
    for syscall in denied {
        filter.extend([
            libc::sock_filter {
                code: BPF_STMT,
                jt: 0,
                jf: 0,
                k: 0,
            },
            libc::sock_filter {
                code: BPF_JMP | BPF_JEQ | BPF_K,
                jt: 0,
                jf: 1,
                k: syscall as u32,
            },
            libc::sock_filter {
                code: BPF_RET | BPF_K,
                jt: 0,
                jf: 0,
                k: SECCOMP_RET_ERRNO | libc::EPERM as u32,
            },
        ]);
    }
    filter.push(libc::sock_filter {
        code: BPF_RET | BPF_K,
        jt: 0,
        jf: 0,
        k: libc::SECCOMP_RET_ALLOW,
    });
    let program = libc::sock_fprog {
        len: filter.len() as libc::c_ushort,
        filter: filter.as_ptr() as *mut libc::sock_filter,
    };
    if unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    if unsafe {
        libc::prctl(
            libc::PR_SET_SECCOMP,
            libc::SECCOMP_MODE_FILTER,
            &program as *const libc::sock_fprog,
            0,
            0,
        )
    } != 0
    {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn macos_sandbox_profile(
    program: &str,
    permissions: &BTreeSet<PluginPermission>,
    filesystem_read_paths: &[PathBuf],
) -> String {
    let mut rules = vec![
        "(version 1)".to_string(),
        "(allow default)".to_string(),
        // Keep the OS/runtime baseline readable, but deny user and temporary
        // data roots unless a signed manifest adds an explicit exception.
        "(deny file-read* (subpath \"/Users\"))".to_string(),
        "(deny file-read* (subpath \"/private/var/folders\"))".to_string(),
        "(deny file-read* (subpath \"/var/folders\"))".to_string(),
        "(deny file-read* (subpath \"/private/tmp\"))".to_string(),
        "(deny file-read* (subpath \"/tmp\"))".to_string(),
        "(allow file-read* (literal \"/dev/null\"))".to_string(),
        // This API has no filesystem-write permission. Keep the standard null
        // sink available because many well-behaved Unix tools use it.
        "(deny file-write*)".to_string(),
        "(allow file-write* (literal \"/dev/null\"))".to_string(),
    ];
    if let Some(program_rule) = sandbox_read_rule("literal", Path::new(program)) {
        rules.push(program_rule);
    }
    if permissions.contains(&PluginPermission::FilesystemRead) {
        for path in filesystem_read_paths {
            if let Some(rule) = sandbox_read_rule("subpath", path) {
                rules.push(rule);
            }
        }
    }
    if !permissions.contains(&PluginPermission::Network) {
        rules.push("(deny network*)".to_string());
    }
    if !permissions.contains(&PluginPermission::ChildProcess) {
        rules.push("(deny process-fork)".to_string());
    }
    rules.join(" ")
}

#[cfg(target_os = "macos")]
fn sandbox_read_rule(kind: &str, path: &Path) -> Option<String> {
    let path = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let path = path.to_str()?.replace('\\', "\\\\").replace('"', "\\\"");
    Some(format!("(allow file-read* ({kind} \"{path}\"))"))
}

fn read_limited(mut reader: impl Read, limit: usize) -> Result<Vec<u8>, std::io::Error> {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 8_192];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            return Ok(bytes);
        }
        if bytes.len().saturating_add(read) > limit {
            return Err(std::io::Error::other("configured output limit exceeded"));
        }
        bytes.extend_from_slice(&buffer[..read]);
    }
}

fn execution_evidence(
    started: Instant,
    status: Option<std::process::ExitStatus>,
    timed_out: bool,
    terminated_process_group: bool,
    stdout_bytes: usize,
    stderr_bytes: usize,
) -> PluginExecutionEvidence {
    PluginExecutionEvidence {
        elapsed_ms: started.elapsed().as_millis().try_into().unwrap_or(u64::MAX),
        exit_code: status.and_then(|status| status.code()),
        timed_out,
        terminated_process_group,
        stdout_bytes,
        stderr_bytes,
    }
}

fn terminate_plugin_process(
    child: &mut std::process::Child,
    pid: u32,
    job_handle: Option<PluginJobHandle>,
) -> Result<std::process::ExitStatus, std::io::Error> {
    #[cfg(windows)]
    let _ = pid;
    #[cfg(unix)]
    {
        let group_id = i32::try_from(pid)
            .map_err(|_| std::io::Error::other("plugin process ID exceeds Unix PID range"))?;
        if unsafe { libc::kill(-group_id, libc::SIGKILL) } == -1 {
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() != Some(libc::ESRCH) {
                return Err(error);
            }
        }
    }
    #[cfg(windows)]
    {
        if let Some(job) = job_handle {
            // SAFETY: the handle was created and assigned by this process.
            unsafe {
                windows_sys::Win32::System::JobObjects::TerminateJobObject(job, 1);
            }
        } else {
            child.kill()?;
        }
    }
    #[cfg(not(any(unix, windows)))]
    child.kill()?;
    let status = child.wait();
    close_windows_job(job_handle);
    status
}

#[cfg(windows)]
fn create_windows_job(
    child: &std::process::Child,
) -> Result<Option<PluginJobHandle>, std::io::Error> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::{
        Foundation::{CloseHandle, HANDLE},
        System::JobObjects::{AssignProcessToJobObject, CreateJobObjectW},
    };
    // SAFETY: null attributes/name request a private unnamed Job Object.
    let job = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
    if job.is_null() {
        return Err(std::io::Error::last_os_error());
    }
    let process = child.as_raw_handle() as HANDLE;
    // SAFETY: process is the live child handle and job is owned above.
    if unsafe { AssignProcessToJobObject(job, process) } == 0 {
        unsafe { CloseHandle(job) };
        return Err(std::io::Error::last_os_error());
    }
    Ok(Some(job))
}

#[cfg(not(windows))]
fn create_windows_job(
    _child: &std::process::Child,
) -> Result<Option<PluginJobHandle>, std::io::Error> {
    Ok(None)
}

#[cfg(windows)]
fn close_windows_job(job_handle: Option<PluginJobHandle>) {
    if let Some(job) = job_handle {
        // SAFETY: job is a handle created by create_windows_job.
        unsafe { windows_sys::Win32::Foundation::CloseHandle(job) };
    }
}

#[cfg(not(windows))]
fn close_windows_job(_job_handle: Option<PluginJobHandle>) {}

#[derive(Debug, Clone, PartialEq)]
pub enum PluginTickOutcome {
    Accepted(Vec<StateDiff>),
    Failed(PluginFailure),
}

#[derive(Debug, Error)]
pub enum PluginError {
    #[error("manifest parse failed: {0}")]
    ManifestParse(String),
    #[error("manifest field '{0}' is invalid")]
    InvalidField(String),
    #[error("plugin hash mismatch: expected {expected}, actual {actual}")]
    HashMismatch { expected: String, actual: String },
    #[error("plugin API contract {actual} is incompatible with {expected}")]
    ApiMismatch { expected: u32, actual: u32 },
    #[error("plugin permission is not allowed: {0:?}")]
    PermissionDenied(PluginPermission),
    #[error("plugin signature is required")]
    SignatureRequired,
    #[error("invalid state diff: {0}")]
    InvalidStateDiff(String),
    #[error("failed to read plugin manifest: {0}")]
    Io(String),
}

#[derive(Debug, Clone)]
pub struct PluginPolicy {
    pub api_contract: u32,
    pub allowed_permissions: BTreeSet<PluginPermission>,
    pub require_signature: bool,
    pub failure_policy: PluginFailurePolicy,
    /// Cooperative per-tick wall-clock budget in milliseconds. A plugin whose
    /// `tick` returns after this budget is treated as a failure and handled by
    /// `failure_policy`. `None` disables the budget.
    ///
    /// This is a cooperative budget: it bounds plugins that return but does not
    /// preempt a hung plugin. OS-level preemption requires out-of-process
    /// execution; see `PluginFailurePolicy` and the contract tests in
    /// `tests/contract/plugin_host.rs` for the enforced failure behavior.
    pub tick_budget_ms: Option<u64>,
}

impl Default for PluginPolicy {
    fn default() -> Self {
        Self {
            api_contract: PLUGIN_API_VERSION,
            allowed_permissions: [PluginPermission::WorldRead].into_iter().collect(),
            require_signature: false,
            failure_policy: PluginFailurePolicy::DisablePlugin,
            tick_budget_ms: Some(50),
        }
    }
}

#[derive(Debug, Clone)]
pub struct LoadedPlugin {
    pub manifest: PluginManifest,
    pub status: PluginStatus,
}

#[derive(Debug, Default)]
pub struct PluginHost {
    plugins: BTreeMap<String, LoadedPlugin>,
    failures: Vec<PluginFailure>,
}

impl PluginHost {
    pub fn discover(
        &mut self,
        directory: impl AsRef<Path>,
        policy: &PluginPolicy,
    ) -> Vec<PluginFailure> {
        let mut failures = Vec::new();
        let entries = match fs::read_dir(directory.as_ref()) {
            Ok(entries) => entries,
            Err(error) => {
                failures.push(self.failure(
                    "<directory>",
                    "unknown",
                    error.to_string(),
                    None,
                    policy,
                ));
                return failures;
            }
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !matches!(
                path.extension().and_then(|value| value.to_str()),
                Some("yaml" | "yml" | "json")
            ) {
                continue;
            }
            match fs::read(&path)
                .map_err(|error| PluginError::Io(error.to_string()))
                .and_then(|bytes| parse_manifest(&bytes))
                .and_then(|manifest| validate_manifest(manifest, policy))
            {
                Ok(manifest) => {
                    self.plugins.insert(
                        manifest.id.clone(),
                        LoadedPlugin {
                            manifest,
                            status: PluginStatus::Ready,
                        },
                    );
                }
                Err(error) => {
                    let id = path
                        .file_stem()
                        .and_then(|value| value.to_str())
                        .unwrap_or("unknown");
                    let failure = self.failure(id, "unknown", error.to_string(), None, policy);
                    self.failures.push(failure.clone());
                    failures.push(failure);
                }
            }
        }
        failures
    }

    pub fn validate_state_diff(
        &self,
        snapshot: &WorldSnapshot,
        diff: &StateDiff,
    ) -> Result<(), PluginError> {
        let plugin = self
            .plugins
            .get(&diff.plugin_id)
            .ok_or_else(|| PluginError::InvalidStateDiff("plugin is not ready".to_string()))?;
        if !plugin
            .manifest
            .permissions
            .contains(&PluginPermission::WorldWrite)
        {
            return Err(PluginError::PermissionDenied(PluginPermission::WorldWrite));
        }
        if diff.expected_state_version != snapshot.version {
            return Err(PluginError::InvalidStateDiff(
                "state version conflict".to_string(),
            ));
        }
        let (entity_id, component_path) = diff.patch.target_key();
        StatePatchTarget::parse(entity_id, component_path)
            .filter(|target| target.value_is_valid(diff.patch.value()))
            .ok_or_else(|| {
                PluginError::InvalidStateDiff(
                    "component path or value is outside plugin write scope".to_string(),
                )
            })
            .map(|_| ())
    }

    pub fn run_tick(
        &mut self,
        plugin_id: &str,
        snapshot: &WorldSnapshot,
        executor: &mut dyn PluginExecutor,
        policy: &PluginPolicy,
    ) -> PluginTickOutcome {
        let Some(plugin) = self.plugins.get(plugin_id) else {
            return PluginTickOutcome::Failed(self.record_failure(
                plugin_id,
                "unknown",
                "plugin is not ready".to_string(),
                None,
                policy,
            ));
        };
        let version = plugin.manifest.version.clone();
        if plugin.status != PluginStatus::Ready {
            return PluginTickOutcome::Failed(self.record_failure(
                plugin_id,
                &version,
                "plugin is not ready".to_string(),
                None,
                policy,
            ));
        }

        let started = std::time::Instant::now();
        let tick_result = executor.tick(snapshot);
        let execution = executor.take_execution_evidence();
        let diffs = match tick_result {
            Ok(diffs) => diffs,
            Err(reason) => {
                return PluginTickOutcome::Failed(self.record_failure(
                    plugin_id,
                    &version,
                    reason,
                    execution.clone(),
                    policy,
                ));
            }
        };
        if let Some(budget_ms) = policy.tick_budget_ms {
            let elapsed_ms = started.elapsed().as_millis();
            if elapsed_ms > u128::from(budget_ms) {
                return PluginTickOutcome::Failed(self.record_failure(
                    plugin_id,
                    &version,
                    format!("plugin tick exceeded {budget_ms}ms budget ({elapsed_ms}ms)"),
                    execution.clone(),
                    policy,
                ));
            }
        }
        for diff in &diffs {
            if diff.plugin_id != plugin_id {
                return PluginTickOutcome::Failed(self.record_failure(
                    plugin_id,
                    &version,
                    "plugin returned a StateDiff for another plugin".to_string(),
                    execution.clone(),
                    policy,
                ));
            }
            if let Err(error) = self.validate_state_diff(snapshot, diff) {
                return PluginTickOutcome::Failed(self.record_failure(
                    plugin_id,
                    &version,
                    error.to_string(),
                    execution.clone(),
                    policy,
                ));
            }
        }
        PluginTickOutcome::Accepted(diffs)
    }

    pub fn get(&self, plugin_id: &str) -> Option<&LoadedPlugin> {
        self.plugins.get(plugin_id)
    }

    pub fn manifests(&self) -> impl Iterator<Item = &PluginManifest> {
        self.plugins.values().map(|plugin| &plugin.manifest)
    }

    pub fn plugin_ids(&self) -> impl Iterator<Item = &str> {
        self.plugins.keys().map(String::as_str)
    }

    pub fn failures(&self) -> &[PluginFailure] {
        &self.failures
    }

    fn failure(
        &self,
        plugin_id: &str,
        version: &str,
        reason: String,
        execution: Option<PluginExecutionEvidence>,
        policy: &PluginPolicy,
    ) -> PluginFailure {
        PluginFailure {
            plugin_id: plugin_id.to_string(),
            version: version.to_string(),
            reason,
            decision: policy.failure_policy.clone(),
            execution,
        }
    }

    fn record_failure(
        &mut self,
        plugin_id: &str,
        version: &str,
        reason: String,
        execution: Option<PluginExecutionEvidence>,
        policy: &PluginPolicy,
    ) -> PluginFailure {
        let failure = self.failure(plugin_id, version, reason, execution, policy);
        if let Some(plugin) = self.plugins.get_mut(plugin_id) {
            plugin.status = match policy.failure_policy {
                PluginFailurePolicy::DisablePlugin => PluginStatus::Disabled,
                PluginFailurePolicy::PauseRun | PluginFailurePolicy::FailRun => {
                    PluginStatus::Failed
                }
            };
        }
        self.failures.push(failure.clone());
        failure
    }
}

fn parse_manifest(bytes: &[u8]) -> Result<PluginManifest, PluginError> {
    let text = std::str::from_utf8(bytes)
        .map_err(|error| PluginError::ManifestParse(error.to_string()))?;
    if text.trim_start().starts_with('{') {
        serde_json::from_str(text).map_err(|error| PluginError::ManifestParse(error.to_string()))
    } else {
        serde_yaml::from_str(text).map_err(|error| PluginError::ManifestParse(error.to_string()))
    }
}

fn validate_manifest(
    mut manifest: PluginManifest,
    policy: &PluginPolicy,
) -> Result<PluginManifest, PluginError> {
    if manifest.id.trim().is_empty() {
        return Err(PluginError::InvalidField("id".to_string()));
    }
    if manifest.version.trim().is_empty() {
        return Err(PluginError::InvalidField("version".to_string()));
    }
    if manifest
        .command
        .as_ref()
        .is_some_and(|command| command.is_empty() || command[0].trim().is_empty())
    {
        return Err(PluginError::InvalidField("command".to_string()));
    }
    if manifest.api_contract != policy.api_contract {
        return Err(PluginError::ApiMismatch {
            expected: policy.api_contract,
            actual: manifest.api_contract,
        });
    }
    if policy.require_signature && manifest.signature.as_deref().unwrap_or("").is_empty() {
        return Err(PluginError::SignatureRequired);
    }
    for permission in &manifest.permissions {
        if !policy.allowed_permissions.contains(permission) {
            return Err(PluginError::PermissionDenied(permission.clone()));
        }
    }
    if !manifest.filesystem_read_paths.is_empty()
        && !manifest
            .permissions
            .contains(&PluginPermission::FilesystemRead)
    {
        return Err(PluginError::PermissionDenied(
            PluginPermission::FilesystemRead,
        ));
    }
    if manifest.filesystem_read_paths.iter().any(|path| {
        let path = Path::new(path);
        !path.is_absolute()
            || path
                .components()
                .any(|component| matches!(component, std::path::Component::ParentDir))
    }) {
        return Err(PluginError::InvalidField("filesystemReadPaths".to_string()));
    }
    let expected = manifest.hash.clone();
    manifest.hash.clear();
    let canonical = serde_json::to_vec(&manifest)
        .map_err(|error| PluginError::ManifestParse(error.to_string()))?;
    let mut hasher = Sha256::new();
    hasher.update(canonical);
    let actual = format!("sha256:{:x}", hasher.finalize());
    if expected != actual {
        return Err(PluginError::HashMismatch { expected, actual });
    }
    manifest.hash = actual;
    Ok(manifest)
}
