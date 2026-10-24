//! Deterministic video facts for rendering boundary tests.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use ghost_cms_core::filesystem::{FileMetadata, FileSystem, Files, StateLease};
use ghost_cms_core::video::VideoMetadata;

#[derive(Debug)]
struct VideoFiles(Files);

impl FileSystem for VideoFiles {
    fn prepare_video(&self, _path: &Path) -> ghost_cms_core::Result<VideoMetadata> {
        VideoMetadata::new(360, 640, 10_540, 360, 640, vec![0xff, 0xd8, 0xff, 0xd9])
    }
    fn canonicalize(&self, path: &Path) -> io::Result<PathBuf> {
        self.0.canonicalize(path)
    }
    fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        self.0.read(path)
    }
    fn read_text(&self, path: &Path) -> io::Result<String> {
        self.0.read_text(path)
    }
    fn metadata(&self, path: &Path) -> io::Result<FileMetadata> {
        self.0.metadata(path)
    }
    fn create_dir(&self, path: &Path) -> io::Result<()> {
        self.0.create_dir(path)
    }
    fn create_dir_all(&self, path: &Path) -> io::Result<()> {
        self.0.create_dir_all(path)
    }
    fn write(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        self.0.write(path, bytes)
    }
    fn remove_dir_all(&self, path: &Path) -> io::Result<()> {
        self.0.remove_dir_all(path)
    }
    fn replace_state(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        self.0.replace_state(path, bytes)
    }
    fn lock_state(&self, path: &Path) -> io::Result<Box<dyn StateLease>> {
        self.0.lock_state(path)
    }
}

/// Supply measured portrait-video facts without invoking a decoder.
#[must_use]
pub fn video_files() -> Files {
    Arc::new(VideoFiles(ghost_cms_shared::filesystem::local_files()))
}
