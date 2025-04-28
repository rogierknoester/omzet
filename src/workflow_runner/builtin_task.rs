use std::path::Path;

use ez_ffmpeg::stream_info::{find_video_stream_info, StreamInfo};
use tracing::warn;

use crate::{
    job_orchestration::TaskReport,
    workflow::{BuiltinTask, Task},
};

use super::common::{ProbeResult, ProbeRunner, ProbingContext, TaskRunner};

impl ProbeRunner for BuiltinTask {
    fn run_probe(&self, context: ProbingContext) -> ProbeResult {
        match self {
            BuiltinTask::TranscodeToH265 => get_codec_name(context.path)
                .map(|codec| match codec.as_str() {
                    "hevc" => ProbeResult::Skip,
                    _ => ProbeResult::Run,
                })
                .unwrap_or(ProbeResult::Abort),
        }
    }
}

impl TaskRunner for BuiltinTask {
    fn run_task(
        &self,
        context: super::common::TaskContext,
    ) -> crate::job_orchestration::TaskReport {
        warn!("running builtin tasks not implemented yet");
        TaskReport::new(Some(1), String::new(), String::new())
    }
}

#[derive(thiserror::Error, Debug)]
enum CodecError {
    #[error(transparent)]
    Ffmpeg(#[from] ez_ffmpeg::error::Error),
    #[error("received unexpected stream for context")]
    UnexpectedStream,
    #[error("unknown error while getting codec")]
    Unknown,
}

/// Get the human readable codec name
fn get_codec_name(path: &Path) -> Result<String, CodecError> {
    let result = find_video_stream_info(path.to_string_lossy())?;

    match result {
        Some(stream_info) => match stream_info {
            StreamInfo::Video { codec_name, .. } => Ok(codec_name),
            _ => Err(CodecError::UnexpectedStream),
        },
        None => Err(CodecError::Unknown),
    }
}
