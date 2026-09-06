//! ffmpeg / ffprobe 通用能力：可执行文件查找、时长探测、视频截帧。
//! 与具体业务（AI 生成等）无关；所有 ffmpeg 进程调用统一在此串行排队，
//! 避免并发任务同时拉起多个 ffmpeg / ffprobe 进程抢占 CPU 与磁盘 IO。

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use tokio::process::Command as TokioCommand;
use tokio::sync::{Mutex, MutexGuard};
use tracing::{info, warn};

use crate::error::AppError;

/// ffmpeg 串行执行队列：tokio 的 Mutex 是 FIFO 公平锁，先到的调用方先执行。
static FFMPEG_QUEUE: OnceLock<Mutex<()>> = OnceLock::new();

/// 进入 ffmpeg 串行队列；返回的 guard 释放后，下一个排队者才会开始执行
pub async fn ffmpeg_queue_lock() -> MutexGuard<'static, ()> {
    FFMPEG_QUEUE.get_or_init(|| Mutex::new(())).lock().await
}

/// 查找 ffmpeg 可执行文件：优先使用配置路径，其次搜索 PATH 与常见安装目录
pub fn resolve_ffmpeg(configured: &str) -> Option<PathBuf> {
    let configured = configured.trim();
    if !configured.is_empty() {
        let path = PathBuf::from(configured);
        if path.is_file() {
            return Some(path);
        }
        warn!("配置的 ffmpeg 路径无效，尝试自动搜索: {configured}");
    }

    let exe_name = if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" };

    // 搜索系统 PATH
    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            let candidate = dir.join(exe_name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    // Windows 常见安装位置
    #[cfg(windows)]
    {
        let mut candidates = vec![
            PathBuf::from(r"C:\ffmpeg\bin").join(exe_name),
            PathBuf::from(r"D:\ffmpeg\bin").join(exe_name),
            PathBuf::from(r"C:\Program Files\ffmpeg\bin").join(exe_name),
            PathBuf::from(r"C:\Program Files (x86)\ffmpeg\bin").join(exe_name),
        ];
        if let Ok(user_profile) = std::env::var("USERPROFILE") {
            // Scoop / WinGet 等包管理器的常见安装位置
            candidates.push(PathBuf::from(&user_profile).join("scoop/shims").join(exe_name));
            candidates.push(
                PathBuf::from(&user_profile)
                    .join("scoop/apps/ffmpeg/current/bin")
                    .join(exe_name),
            );
            candidates.push(
                PathBuf::from(&user_profile)
                    .join("AppData/Local/Microsoft/WinGet/Links")
                    .join(exe_name),
            );
        }
        for candidate in candidates {
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    // macOS / Linux 常见安装位置
    #[cfg(not(windows))]
    {
        for candidate in [
            "/usr/bin/ffmpeg",
            "/usr/local/bin/ffmpeg",
            "/opt/homebrew/bin/ffmpeg",
            "/snap/bin/ffmpeg",
        ] {
            let path = PathBuf::from(candidate);
            if path.is_file() {
                return Some(path);
            }
        }
    }

    None
}

/// 截取视频末尾附近的一帧，返回 (时间点描述, JPEG 字节)。
///
/// 优先用 `-sseof` 从文件末尾倒数定位：只需一次 ffmpeg 调用、一次文件解析；
/// 失败（部分容器不支持从末尾 seek）时回退为「探测时长 + `-ss` 精确定位」。
/// 各阶段耗时都会写入 info 日志，便于定位长视频截帧慢的原因。
pub async fn capture_frame_near_end(
    ffmpeg: &Path,
    video_path: &str,
    from_end_secs: f64,
) -> Result<(String, Vec<u8>), AppError> {
    let total_start = Instant::now();

    // 1. 优先：从末尾倒数定位，省掉一次 ffprobe / ffmpeg 的全文件解析
    match extract_frame_from_end(ffmpeg, video_path, from_end_secs).await {
        Ok(bytes) if !bytes.is_empty() => {
            info!(
                "截帧完成（末尾倒数定位）: {} 字节, 总耗时 {:.3}s, 路径: {video_path}",
                bytes.len(),
                total_start.elapsed().as_secs_f64()
            );
            return Ok((format!("末尾 {from_end_secs:.0}s 处"), bytes));
        }
        Ok(_) => warn!("末尾倒数定位返回空帧，回退到「探测时长 + 精确定位」: {video_path}"),
        Err(e) => warn!("末尾倒数定位失败，回退到「探测时长 + 精确定位」: {e}"),
    }

    // 2. 回退：探测时长后精确定位
    let probe_start = Instant::now();
    let duration = probe_video_duration(ffmpeg, video_path).await?;
    info!(
        "视频时长: {duration:.3}s (探测耗时 {:.3}s), 路径: {video_path}",
        probe_start.elapsed().as_secs_f64()
    );

    let target = (duration - from_end_secs).max(0.0);
    let extract_start = Instant::now();
    let bytes = extract_frame(ffmpeg, video_path, target).await?;
    info!(
        "截帧完成（精确定位）: {} 字节, 时间点 {target:.3}s, 耗时 {:.3}s",
        bytes.len(),
        extract_start.elapsed().as_secs_f64()
    );
    Ok((format!("{target:.3}s"), bytes))
}

/// 探测视频时长（秒）。优先使用同目录 ffprobe，失败时解析 ffmpeg -i 输出
pub async fn probe_video_duration(ffmpeg: &Path, video_path: &str) -> Result<f64, AppError> {
    // 优先使用 ffprobe（通常与 ffmpeg 同目录安装）
    let ffprobe = ffmpeg.with_file_name(if cfg!(windows) { "ffprobe.exe" } else { "ffprobe" });
    if ffprobe.is_file() {
        let (stdout, stderr) = run_process(
            &ffprobe,
            &[
                "-v",
                "error",
                "-show_entries",
                "format=duration",
                "-of",
                "default=noprint_wrappers=1:nokey=1",
                video_path,
            ],
            30,
        )
        .await?;
        let text = String::from_utf8_lossy(&stdout).trim().to_string();
        if let Ok(secs) = text.parse::<f64>() {
            return Ok(secs);
        }
        warn!("ffprobe 解析时长失败: {text:?} | {stderr}");
    }

    // 回退：解析 ffmpeg -i 输出的 Duration 字段
    let (_stdout, stderr) = run_process(ffmpeg, &["-hide_banner", "-i", video_path], 30).await?;
    if let Some(secs) = parse_ffmpeg_duration(&stderr) {
        return Ok(secs);
    }

    Err(AppError::Custom(
        "无法获取视频时长（视频文件可能损坏或编码不受支持）。".to_string(),
    ))
}

/// 截取指定时间点的视频帧，返回 JPEG 图片字节。
/// `-ss` 放在 `-i` 之前属于输入侧快速定位（跳到最近关键帧再解码到目标点），
/// 比放在 `-i` 之后的输出侧定位快得多。
pub async fn extract_frame(
    ffmpeg: &Path,
    video_path: &str,
    at_second: f64,
) -> Result<Vec<u8>, AppError> {
    let seek_arg = format!("{at_second:.3}");
    let (stdout, stderr) = run_process(
        ffmpeg,
        &[
            "-y",
            "-ss",
            &seek_arg,
            "-i",
            video_path,
            // 只要一帧画面，禁用音频 / 字幕 / 数据流解码，避免做无用功
            "-frames:v",
            "1",
            "-an",
            "-sn",
            "-dn",
            "-f",
            "image2pipe",
            "-c:v",
            "mjpeg",
            "-q:v",
            "5",
            "pipe:1",
        ],
        60,
    )
    .await?;

    if stdout.is_empty() {
        return Err(AppError::Custom(format!(
            "截取视频画面失败: {}",
            stderr.lines().next_back().unwrap_or("未知错误")
        )));
    }
    Ok(stdout)
}

/// 从文件末尾倒数 `from_end_secs` 秒处截帧（`-sseof`），一次调用即可完成。
/// 目标是「最后几秒的画面」时首选：省掉一次时长探测带来的额外进程与文件解析。
async fn extract_frame_from_end(
    ffmpeg: &Path,
    video_path: &str,
    from_end_secs: f64,
) -> Result<Vec<u8>, AppError> {
    let seek_arg = format!("-{from_end_secs:.3}");
    let (stdout, stderr) = run_process(
        ffmpeg,
        &[
            "-y",
            "-sseof",
            &seek_arg,
            "-i",
            video_path,
            "-frames:v",
            "1",
            "-an",
            "-sn",
            "-dn",
            "-f",
            "image2pipe",
            "-c:v",
            "mjpeg",
            "-q:v",
            "5",
            "pipe:1",
        ],
        60,
    )
    .await?;

    if stdout.is_empty() {
        return Err(AppError::Custom(format!(
            "sseof 截帧无输出: {}",
            stderr.lines().next_back().unwrap_or("未知错误")
        )));
    }
    Ok(stdout)
}

/// 执行外部进程并捕获 stdout / stderr（同时记录进程耗时，便于排查慢在哪一步）
async fn run_process(
    program: &Path,
    args: &[&str],
    timeout_secs: u64,
) -> Result<(Vec<u8>, String), AppError> {
    let started = Instant::now();

    let mut cmd = TokioCommand::new(program);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let child = cmd
        .spawn()
        .map_err(|e| AppError::Custom(format!("无法启动 {}: {e}", program.display())))?;

    let output = tokio::time::timeout(Duration::from_secs(timeout_secs), child.wait_with_output())
        .await
        .map_err(|_| AppError::Custom(format!("{} 执行超时", program.display())))?
        .map_err(|e| AppError::Custom(format!("执行 {} 失败: {e}", program.display())))?;

    info!(
        "外部进程耗时 {:.3}s: {} {}",
        started.elapsed().as_secs_f64(),
        program.display(),
        args.join(" ")
    );

    Ok((
        output.stdout,
        String::from_utf8_lossy(&output.stderr).into_owned(),
    ))
}

/// 从 ffmpeg -i 的 stderr 输出中解析 Duration: HH:MM:SS.xx
fn parse_ffmpeg_duration(stderr: &str) -> Option<f64> {
    const MARKER: &str = "Duration: ";
    let pos = stderr.find(MARKER)?;
    let rest = &stderr[pos + MARKER.len()..];
    let token = rest.split(',').next().unwrap_or("").trim();
    if token.is_empty() || token.eq_ignore_ascii_case("N/A") {
        return None;
    }

    let parts: Vec<&str> = token.split(':').collect();
    if parts.len() != 3 {
        return None;
    }
    let hours: f64 = parts[0].trim().parse().ok()?;
    let minutes: f64 = parts[1].trim().parse().ok()?;
    let seconds: f64 = parts[2].trim().parse().ok()?;
    Some(hours * 3600.0 + minutes * 60.0 + seconds)
}
