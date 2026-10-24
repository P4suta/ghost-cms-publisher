use std::path::Path;
use std::process::{Command, Output};

use ghost_cms_core::video::VideoMetadata;
use ghost_cms_core::{CoreError, Result};
use serde::Deserialize;

const MAX_FRAME_PIXELS: u64 = 33_177_600;
const MILLISECONDS_PER_SECOND: f64 = 1_000.0;
const FULL_TURN: i32 = 360;
const HALF_TURN: i32 = 180;
const QUARTER_TURN: i32 = 90;
const THREE_QUARTER_TURN: i32 = 270;
const THUMBNAIL_FILTER: &str =
    "scale='min(960,iw)':'min(960,ih)':force_original_aspect_ratio=decrease";

#[derive(Deserialize)]
struct Probe {
    streams: Vec<Stream>,
    #[serde(default)]
    format: Format,
}

#[derive(Default, Deserialize)]
struct Format {
    duration: Option<String>,
}

#[derive(Deserialize)]
struct Stream {
    width: u32,
    height: u32,
    duration: Option<String>,
    #[serde(default)]
    side_data_list: Vec<SideData>,
}

#[derive(Deserialize)]
struct SideData {
    rotation: Option<i32>,
}

fn checked(command: &mut Command) -> Result<Output> {
    let output = command.output().map_err(|error| {
        CoreError::Config(format!(
            "video preparation requires FFmpeg and ffprobe on PATH: {error}"
        ))
    })?;
    if !output.status.success() {
        return Err(CoreError::Config(format!(
            "video decoding failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(output)
}

fn probe(path: &Path) -> Result<Probe> {
    Ok(serde_json::from_slice(
        &checked(
            Command::new("ffprobe")
                .args([
                    "-v",
                    "error",
                    "-protocol_whitelist",
                    "file,pipe",
                    "-threads",
                    "1",
                    "-select_streams",
                    "v:0",
                    "-show_entries",
                    "stream=width,height,duration:stream_side_data=rotation:format=duration",
                    "-of",
                    "json",
                ])
                .arg(path),
        )?
        .stdout,
    )?)
}

pub(super) fn prepare(path: &Path) -> Result<VideoMetadata> {
    let source = probe(path)?;
    let stream = source
        .streams
        .first()
        .ok_or_else(|| CoreError::Config("video has no video stream".to_owned()))?;
    let duration = stream
        .duration
        .as_ref()
        .or(source.format.duration.as_ref())
        .ok_or_else(|| CoreError::Config("video has no measured duration".to_owned()))?
        .parse::<f64>()
        .map_err(|error| CoreError::Config(format!("invalid video duration: {error}")))?;
    if !duration.is_finite()
        || duration <= 0.0
        || duration * MILLISECONDS_PER_SECOND > f64::from(u32::MAX)
        || u64::from(stream.width) * u64::from(stream.height) > MAX_FRAME_PIXELS
    {
        return Err(CoreError::Config(
            "video duration or decoded frame size is outside the supported range".to_owned(),
        ));
    }
    let duration_ms = std::time::Duration::try_from_secs_f64(duration)
        .map_err(|error| CoreError::Config(format!("invalid video duration: {error}")))?
        .as_millis();
    let duration_ms =
        u32::try_from(duration_ms).map_err(|error| CoreError::Config(error.to_string()))?;
    let rotation = stream
        .side_data_list
        .iter()
        .find_map(|data| data.rotation)
        .unwrap_or(0)
        .rem_euclid(FULL_TURN);
    let (width, height) = match rotation {
        0 | HALF_TURN => (stream.width, stream.height),
        QUARTER_TURN | THREE_QUARTER_TURN => (stream.height, stream.width),
        _ => {
            return Err(CoreError::Config(
                "video rotation must use a right angle".to_owned(),
            ));
        },
    };
    let temp = tempfile::tempdir()?;
    let frame = temp.path().join("frame.jpg");
    checked(
        Command::new("ffmpeg")
            .args([
                "-v",
                "error",
                "-nostdin",
                "-protocol_whitelist",
                "file,pipe",
                "-threads",
                "1",
                "-i",
            ])
            .arg(path)
            .args([
                "-map",
                "0:v:0",
                "-frames:v",
                "1",
                "-an",
                "-sn",
                "-vf",
                THUMBNAIL_FILTER,
                "-filter_threads",
                "1",
                "-threads",
                "1",
                "-q:v",
                "2",
                "-update",
                "1",
            ])
            .arg(&frame),
    )?;
    let thumbnail = probe(&frame)?;
    let thumbnail = thumbnail
        .streams
        .first()
        .ok_or_else(|| CoreError::Config("decoded thumbnail has no dimensions".to_owned()))?;
    VideoMetadata::new(
        width,
        height,
        duration_ms,
        thumbnail.width,
        thumbnail.height,
        std::fs::read(&frame)?,
    )
}
