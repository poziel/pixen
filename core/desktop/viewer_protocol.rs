//! Toolkit-independent managed viewer process and wire types.
use crate::review::ReviewResult;
use anyhow::{Context, Result, ensure};
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::mpsc::{self, Receiver},
    time::Duration,
};
fn viewer_executable(current: &Path) -> Result<PathBuf> {
    let name = if cfg!(windows) { "pixen.exe" } else { "pixen" };
    let path = current
        .parent()
        .context("executable has no parent directory")?
        .join(name);
    ensure!(
        path.is_file(),
        "Pixen app missing: {}; install pixen and pixen-cli together",
        path.display()
    );
    Ok(path)
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Activity {
    pub label: String,
    pub frame_id: Option<String>,
    pub layer_id: Option<String>,
    pub busy: bool,
}
pub struct ViewerProcess {
    child: Child,
    replies: Receiver<Value>,
}
impl ViewerProcess {
    pub fn open(path: &Path) -> Result<(Self, Value)> {
        let mut command = Command::new(viewer_executable(&std::env::current_exe()?)?);
        command
            .arg("--managed")
            .arg(path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let mut child = command
            .spawn()
            .context("start preview app; check executable availability")?;
        let stdout = child
            .stdout
            .take()
            .context("viewer response pipe missing")?;
        let (tx, replies) = mpsc::sync_channel(8);
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else {
                    break;
                };
                let Ok(reply) = serde_json::from_str(&line) else {
                    break;
                };
                if tx.send(reply).is_err() {
                    break;
                }
            }
        });
        let mut process = Self { child, replies };
        let reply = process.reply()?;
        Ok((process, reply))
    }
    fn reply(&mut self) -> Result<Value> {
        let reply=self.replies.recv_timeout(Duration::from_secs(15)).map_err(|error| {
            let message=match error {
                mpsc::RecvTimeoutError::Timeout=>"preview app did not confirm opening within 15 seconds; ensure an interactive desktop and working WebView runtime, then retry open_preview",
                mpsc::RecvTimeoutError::Disconnected=>"preview process disconnected; ensure an interactive desktop and working WebView runtime, then retry open_preview (see stderr for backend details)",
            };
            anyhow::Error::new(error).context(message)
        })?;
        ensure!(
            reply["opened"] == true,
            "preview unavailable: {}",
            reply["error"]
                .as_str()
                .unwrap_or("viewer disconnected; retry open_preview from an interactive desktop")
        );
        Ok(reply)
    }
    pub fn alive(&mut self) -> bool {
        self.child
            .try_wait()
            .ok()
            .is_some_and(|status| status.is_none())
    }
    pub fn activate(&mut self) -> Result<Value> {
        ensure!(
            self.alive(),
            "preview window has closed; call open_preview again"
        );
        writeln!(
            self.child
                .stdin
                .as_mut()
                .context("preview command pipe closed")?,
            "activate"
        )
        .context("viewer disconnected; retry open_preview")?;
        self.reply()
    }
    pub(crate) fn activity(&mut self, activity: &Activity) -> Result<()> {
        let input = self.child.stdin.as_mut().context("viewer input closed")?;
        serde_json::to_writer(&mut *input, activity)?;
        writeln!(input)?;
        input.flush()?;
        Ok(())
    }
    pub(crate) fn inspection(&mut self, view: &crate::inspection::View) -> Result<()> {
        let input = self.child.stdin.as_mut().context("viewer input closed")?;
        serde_json::to_writer(&mut *input, &json!({"inspection":view}))?;
        writeln!(input)?;
        input.flush()?;
        Ok(())
    }
    pub(crate) fn review(&mut self, review: &ViewerReview) -> Result<()> {
        let input = self.child.stdin.as_mut().context("viewer input closed")?;
        serde_json::to_writer(&mut *input, &json!({"review":review}))?;
        writeln!(input)?;
        input.flush()?;
        Ok(())
    }
    pub fn close(&mut self) {
        drop(self.child.stdin.take());
        for _ in 0..100 {
            if !self.alive() {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
impl Drop for ViewerProcess {
    fn drop(&mut self) {
        self.close();
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WireImage {
    pub role: String,
    pub png: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ViewerReview {
    pub report: Value,
    pub images: Vec<WireImage>,
}
impl ViewerReview {
    pub fn new(result: &ReviewResult) -> Result<Self> {
        ensure!(
            result.images.iter().map(|i| i.png.len()).sum::<usize>() <= 40 * 1024 * 1024,
            "inspector review payload too large; reduce scales/views/frames"
        );
        let r = Self {
            report: result.report.clone(),
            images: result
                .images
                .iter()
                .map(|i| WireImage {
                    role: i.role.clone(),
                    png: base64::engine::general_purpose::STANDARD.encode(&i.png),
                })
                .collect(),
        };
        ensure!(
            serde_json::to_vec(&r)?.len() <= 64 * 1024 * 1024,
            "inspector review exceeds 64 MiB"
        );
        Ok(r)
    }
    pub fn frame_id(&self) -> Option<&str> {
        self.report["frame_id"]
            .as_str()
            .or_else(|| self.report["frames"][0]["frame_id"].as_str())
    }
}
