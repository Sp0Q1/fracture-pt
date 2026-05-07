//! Generic sidecar-container runner for pentest tools.
//!
//! Every external pentest binary (amass, nuclei, sslscan, …) runs in its own
//! pinned podman image. The host invokes the binary by spawning `podman run`
//! with hard limits + minimal capabilities, captures stdout, and stops the
//! container when it exits.
//!
//! ## Why podman, not direct subprocess
//!
//! The CLAUDE.md `Pentest-tool execution` rules forbid invoking external
//! binaries from the app process. Sidecar containers give us per-tool
//! resource limits, dropped capabilities, no host-FS access, and a clean
//! kill switch (the `--rm` flag).
//!
//! ## Input safety
//!
//! - Only argv is passed (no shell). `tokio::process::Command::args` takes
//!   `&[&str]`, so injection through quoting is structurally impossible.
//! - All user-supplied targets must be validated by the *caller* (e.g.
//!   `services::port_scan::validate_target`) before reaching this module.
//!   This module trusts its inputs and is documented as such.
//! - The image tag is pinned by the caller, not interpolated — prevents
//!   image-substitution attacks.
//!
//! ## Resource limits
//!
//! Wall-clock timeout (Tokio) and memory/CPU/pid caps (podman) are belt
//! and suspenders. The Tokio timeout fires if podman itself hangs; the
//! container limits keep a runaway tool from drowning the host.

use std::time::Duration;

use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::time::timeout;

/// What we want the container to do.
#[derive(Debug)]
pub struct RunSpec<'a> {
    /// Fully-qualified, pinned image reference. e.g.
    /// `ghcr.io/sp0q1/fracture-pt-amass:v4.2.0`.
    pub image: &'a str,
    /// Human-readable label for logs only. Not passed to podman.
    pub tool_name: &'a str,
    /// Argv to pass to the container's ENTRYPOINT, post-validation.
    pub args: &'a [&'a str],
    /// Hard wall-clock cap. Fires if podman / the tool hangs.
    pub wall_clock: Duration,
    /// Memory cap (e.g. `"512m"`). Passed verbatim to `--memory`.
    pub memory: &'a str,
    /// CPU cap (e.g. `"1.0"`). Passed verbatim to `--cpus`.
    pub cpus: &'a str,
    /// PID cap. Passed verbatim to `--pids-limit`.
    pub pids: u32,
}

/// What came back.
#[derive(Debug)]
pub struct RunOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
}

/// Errors a runner can surface.
#[derive(Debug, thiserror::Error)]
pub enum RunError {
    #[error("podman not found on PATH; install podman to invoke `{0}`")]
    PodmanMissing(String),
    #[error("`{tool}` exceeded {limit_secs}s wall-clock limit")]
    Timeout { tool: String, limit_secs: u64 },
    #[error("`{0}` failed to spawn: {1}")]
    Spawn(String, std::io::Error),
    #[error("`{0}` produced unreadable output: {1}")]
    Io(String, std::io::Error),
}

/// Spawn a sidecar container, capture stdout/stderr, return when it exits.
///
/// Uses a hardened set of `podman run` flags. Callers must already have
/// validated `spec.args` — this module does not introspect the argv.
///
/// # Errors
///
/// Returns `PodmanMissing` if podman isn't on PATH, `Timeout` if the
/// `wall_clock` deadline fires, `Spawn`/`Io` for OS-level failures.
pub async fn run(spec: RunSpec<'_>) -> Result<RunOutput, RunError> {
    let mut cmd = Command::new("podman");
    cmd.arg("run")
        .arg("--rm")
        .arg("--network=bridge") // tools need DNS / TLS for upstream queries
        .arg("--read-only")
        .arg("--tmpfs=/tmp:rw,size=64m")
        .arg("--cap-drop=ALL")
        .arg("--security-opt=no-new-privileges")
        .arg(format!("--memory={}", spec.memory))
        .arg(format!("--cpus={}", spec.cpus))
        .arg(format!("--pids-limit={}", spec.pids))
        .arg("--log-driver=none") // don't fill /var/lib with tool output
        // Operators must pre-pull/build the pinned image. `--pull=never`
        // prevents podman from contacting a registry at job time, which
        // (a) avoids registry-auth failures masquerading as tool errors,
        // (b) ensures the image actually running matches the operator's
        //     pinned, audited copy — no surprise tag drift.
        .arg("--pull=never")
        .arg(spec.image)
        .args(spec.args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);

    let mut child = cmd.spawn().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            RunError::PodmanMissing(spec.tool_name.to_string())
        } else {
            RunError::Spawn(spec.tool_name.to_string(), e)
        }
    })?;

    let mut stdout_buf = Vec::new();
    let mut stderr_buf = Vec::new();
    let mut child_stdout = child.stdout.take();
    let mut child_stderr = child.stderr.take();

    let limit_secs = spec.wall_clock.as_secs();
    let result = timeout(spec.wall_clock, async {
        let stdout_fut = async {
            if let Some(s) = child_stdout.as_mut() {
                s.read_to_end(&mut stdout_buf).await
            } else {
                Ok(0)
            }
        };
        let stderr_fut = async {
            if let Some(s) = child_stderr.as_mut() {
                s.read_to_end(&mut stderr_buf).await
            } else {
                Ok(0)
            }
        };
        let (out_res, err_res, status) = tokio::join!(stdout_fut, stderr_fut, child.wait());
        out_res?;
        err_res?;
        status
    })
    .await;

    let status = match result {
        Ok(Ok(s)) => s,
        Ok(Err(e)) => return Err(RunError::Io(spec.tool_name.to_string(), e)),
        Err(_) => {
            // The container is still running. kill_on_drop fires on the next
            // line (Child drops); this is belt and suspenders.
            let _ = child.kill().await;
            return Err(RunError::Timeout {
                tool: spec.tool_name.to_string(),
                limit_secs,
            });
        }
    };

    Ok(RunOutput {
        stdout: String::from_utf8_lossy(&stdout_buf).into_owned(),
        stderr: String::from_utf8_lossy(&stderr_buf).into_owned(),
        exit_code: status.code(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_error_messages_are_useful() {
        let err = RunError::Timeout {
            tool: "amass".into(),
            limit_secs: 600,
        };
        let msg = format!("{err}");
        assert!(msg.contains("amass"));
        assert!(msg.contains("600s"));

        let err = RunError::PodmanMissing("amass".into());
        assert!(format!("{err}").contains("amass"));
    }
}
