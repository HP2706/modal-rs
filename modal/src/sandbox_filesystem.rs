use std::collections::HashSet;

use crate::error::ModalError;

const WRITE_CHUNK_SIZE: usize = 16 * 1024 * 1024; // 16 MiB
const WRITE_FILE_SIZE_LIMIT: usize = 1024 * 1024 * 1024; // 1 GiB
const READ_FILE_SIZE_LIMIT: usize = 100 * 1024 * 1024; // 100 MiB

/// System error codes mapping to errno values from the sandbox runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemErrorCode {
    Unspecified = 0,
    PermissionDenied = 1,    // EPERM
    NotFound = 2,            // ENOENT
    IoError = 5,             // EIO
    NoDevice = 6,            // ENXIO
    OutOfMemory = 12,        // ENOMEM
    AccessDenied = 13,       // EACCES
    AlreadyExists = 17,      // EEXIST
    NotDirectory = 20,       // ENOTDIR
    IsDirectory = 21,        // EISDIR
    InvalidArgument = 22,    // EINVAL
    TooManyOpenFiles = 24,   // EMFILE
    FileTooLarge = 27,       // EFBIG
    NoSpaceLeft = 28,        // ENOSPC
}

impl SystemErrorCode {
    /// Convert a proto error code integer to a SystemErrorCode.
    pub fn from_i32(code: i32) -> Self {
        match code {
            1 => Self::PermissionDenied,
            2 => Self::NotFound,
            5 => Self::IoError,
            6 => Self::NoDevice,
            12 => Self::OutOfMemory,
            13 => Self::AccessDenied,
            17 => Self::AlreadyExists,
            20 => Self::NotDirectory,
            21 => Self::IsDirectory,
            22 => Self::InvalidArgument,
            24 => Self::TooManyOpenFiles,
            27 => Self::FileTooLarge,
            28 => Self::NoSpaceLeft,
            _ => Self::Unspecified,
        }
    }

    /// Convert the error code to a ModalError with the given message.
    pub fn to_error(self, message: &str) -> ModalError {
        match self {
            Self::NotFound => ModalError::NotFound(message.to_string()),
            Self::AlreadyExists => ModalError::AlreadyExists(message.to_string()),
            Self::InvalidArgument => ModalError::Invalid(message.to_string()),
            _ => ModalError::SandboxFilesystem(format!("{}: {}", self.description(), message)),
        }
    }

    /// Human-readable description of the error code.
    pub fn description(&self) -> &'static str {
        match self {
            Self::Unspecified => "filesystem error",
            Self::PermissionDenied => "operation not permitted",
            Self::NotFound => "no such file or directory",
            Self::IoError => "input/output error",
            Self::NoDevice => "no such device or address",
            Self::OutOfMemory => "out of memory",
            Self::AccessDenied => "permission denied",
            Self::AlreadyExists => "file exists",
            Self::NotDirectory => "not a directory",
            Self::IsDirectory => "is a directory",
            Self::InvalidArgument => "invalid argument",
            Self::TooManyOpenFiles => "too many open files",
            Self::FileTooLarge => "file too large",
            Self::NoSpaceLeft => "no space left on device",
        }
    }
}

/// Seek position reference point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeekWhence {
    /// Absolute file positioning (offset from start).
    Set = 0,
    /// Relative to current position.
    Current = 1,
    /// Relative to file end.
    End = 2,
}

impl SeekWhence {
    /// Convert from integer whence value (matching Python's io.SEEK_*).
    pub fn from_i32(whence: i32) -> Result<Self, ModalError> {
        match whence {
            0 => Ok(Self::Set),
            1 => Ok(Self::Current),
            2 => Ok(Self::End),
            _ => Err(ModalError::Invalid(format!(
                "invalid whence value: {}",
                whence
            ))),
        }
    }
}

/// File watch event types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileWatchEventType {
    Unknown,
    Access,
    Create,
    Modify,
    Remove,
}

impl FileWatchEventType {
    /// Parse from a string representation.
    pub fn from_str(s: &str) -> Self {
        match s {
            "Access" => Self::Access,
            "Create" => Self::Create,
            "Modify" => Self::Modify,
            "Remove" => Self::Remove,
            _ => Self::Unknown,
        }
    }

    /// Convert to string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Unknown => "Unknown",
            Self::Access => "Access",
            Self::Create => "Create",
            Self::Modify => "Modify",
            Self::Remove => "Remove",
        }
    }
}

/// A file watch event from the sandbox filesystem.
#[derive(Debug, Clone)]
pub struct FileWatchEvent {
    pub paths: Vec<String>,
    pub event_type: FileWatchEventType,
}

impl FileWatchEvent {
    /// Parse a file watch event from JSON bytes.
    pub fn from_json(data: &[u8]) -> Result<Self, ModalError> {
        let event: serde_json::Value = serde_json::from_slice(data)
            .map_err(|e| ModalError::SandboxFilesystem(format!("failed to parse watch event: {}", e)))?;

        let event_type = event
            .get("event_type")
            .and_then(|v| v.as_str())
            .map(FileWatchEventType::from_str)
            .unwrap_or(FileWatchEventType::Unknown);

        let paths = event
            .get("paths")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        Ok(Self { paths, event_type })
    }
}

/// File mode configuration parsed from a mode string (e.g., "r", "wb", "r+").
#[derive(Debug, Clone, Default)]
pub struct FileMode {
    pub readable: bool,
    pub writable: bool,
    pub appended: bool,
    pub binary: bool,
    raw: String,
}

impl FileMode {
    /// Parse and validate a file mode string.
    ///
    /// Valid mode characters: r, w, a, x (exactly one required), b, + (optional).
    /// Examples: "r", "w", "rb", "r+", "wb+", "a"
    pub fn parse(mode: &str) -> Result<Self, ModalError> {
        if mode.is_empty() {
            return Err(ModalError::Invalid("file mode cannot be empty".to_string()));
        }

        let valid_chars: HashSet<char> = "rwaxb+".chars().collect();
        for c in mode.chars() {
            if !valid_chars.contains(&c) {
                return Err(ModalError::Invalid(format!("invalid file mode: {}", mode)));
            }
        }

        // Check for duplicate characters
        let mut seen = HashSet::new();
        for c in mode.chars() {
            if !seen.insert(c) {
                return Err(ModalError::Invalid(format!("invalid file mode: {}", mode)));
            }
        }

        // Must have exactly one of r, w, a, x
        let mode_count = mode.chars().filter(|c| "rwax".contains(*c)).count();
        if mode_count == 0 {
            return Err(ModalError::Invalid(format!("invalid file mode: {}", mode)));
        }
        if mode_count > 1 {
            return Err(ModalError::Invalid(
                "must have exactly one of create/read/write/append mode".to_string(),
            ));
        }

        let has_plus = mode.contains('+');

        Ok(Self {
            readable: mode.contains('r') || has_plus,
            writable: mode.contains('w') || mode.contains('a') || mode.contains('x') || has_plus,
            appended: mode.contains('a'),
            binary: mode.contains('b'),
            raw: mode.to_string(),
        })
    }

    /// Get the raw mode string.
    pub fn as_str(&self) -> &str {
        &self.raw
    }
}

/// FileIO handle for sandbox filesystem operations.
///
/// Provides file I/O operations within a Modal sandbox, including
/// read, write, seek, flush, and close operations.
#[derive(Debug)]
pub struct FileIO {
    pub task_id: String,
    pub file_descriptor: String,
    pub mode: FileMode,
    closed: bool,
}

impl FileIO {
    /// Create a new FileIO handle after a successful open operation.
    pub fn new(task_id: String, file_descriptor: String, mode: FileMode) -> Self {
        Self {
            task_id,
            file_descriptor,
            mode,
            closed: false,
        }
    }

    /// Returns true if the file is closed.
    pub fn is_closed(&self) -> bool {
        self.closed
    }

    /// Mark the file as closed.
    pub fn close(&mut self) {
        self.closed = true;
    }

    /// Check that the file is not closed.
    pub fn check_closed(&self) -> Result<(), ModalError> {
        if self.closed {
            return Err(ModalError::SandboxFilesystem(
                "I/O operation on closed file".to_string(),
            ));
        }
        Ok(())
    }

    /// Check that the file is readable.
    pub fn check_readable(&self) -> Result<(), ModalError> {
        if !self.mode.readable {
            return Err(ModalError::SandboxFilesystem("not readable".to_string()));
        }
        Ok(())
    }

    /// Check that the file is writable.
    pub fn check_writable(&self) -> Result<(), ModalError> {
        if !self.mode.writable {
            return Err(ModalError::SandboxFilesystem("not writable".to_string()));
        }
        Ok(())
    }

    /// Validate a read size against the limit.
    pub fn validate_read_size(n: Option<usize>) -> Result<(), ModalError> {
        if let Some(size) = n {
            if size > READ_FILE_SIZE_LIMIT {
                return Err(ModalError::Invalid(
                    "read request payload exceeds 100 MiB limit".to_string(),
                ));
            }
        }
        Ok(())
    }

    /// Validate write data against the limit.
    pub fn validate_write_size(data: &[u8]) -> Result<(), ModalError> {
        if data.len() > WRITE_FILE_SIZE_LIMIT {
            return Err(ModalError::Invalid(
                "write request payload exceeds 1 GiB limit".to_string(),
            ));
        }
        Ok(())
    }

    /// Split data into chunks for writing.
    pub fn write_chunks(data: &[u8]) -> Vec<&[u8]> {
        data.chunks(WRITE_CHUNK_SIZE).collect()
    }
}

/// Parsed directory listing result from JSON output.
#[derive(Debug, Clone)]
pub struct DirListing {
    pub paths: Vec<String>,
}

impl DirListing {
    /// Parse a directory listing from JSON bytes.
    pub fn from_json(data: &[u8]) -> Result<Self, ModalError> {
        let value: serde_json::Value = serde_json::from_slice(data)
            .map_err(|e| ModalError::SandboxFilesystem(format!("failed to parse list output: {}", e)))?;

        let paths = value
            .get("paths")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .ok_or_else(|| {
                ModalError::SandboxFilesystem("failed to parse list output".to_string())
            })?;

        Ok(Self { paths })
    }
}

/// SandboxFilesystemService provides file I/O operations on sandbox containers.
///
/// This trait abstracts the gRPC calls needed for filesystem operations,
/// allowing mock implementations for testing.
pub trait SandboxFilesystemService: Send + Sync {
    /// Open a file in the sandbox.
    fn open(&self, task_id: &str, path: &str, mode: &str) -> Result<FileIO, ModalError>;

    /// Read up to `n` bytes from a file. If `n` is None, read the entire file.
    fn read(&self, file_io: &FileIO, n: Option<usize>) -> Result<Vec<u8>, ModalError>;

    /// Read a single line from a file.
    fn readline(&self, file_io: &FileIO) -> Result<Vec<u8>, ModalError>;

    /// Write data to a file.
    fn write(&self, file_io: &FileIO, data: &[u8]) -> Result<(), ModalError>;

    /// Flush the file buffer to disk.
    fn flush(&self, file_io: &FileIO) -> Result<(), ModalError>;

    /// Seek to a position in the file.
    fn seek(&self, file_io: &FileIO, offset: i64, whence: SeekWhence) -> Result<(), ModalError>;

    /// Close a file.
    fn close(&self, file_io: &mut FileIO) -> Result<(), ModalError>;

    /// List directory contents.
    fn ls(&self, task_id: &str, path: &str) -> Result<Vec<String>, ModalError>;

    /// Create a directory.
    fn mkdir(&self, task_id: &str, path: &str, parents: bool) -> Result<(), ModalError>;

    /// Remove a file or directory.
    fn rm(&self, task_id: &str, path: &str, recursive: bool) -> Result<(), ModalError>;
}

/// Trait abstracting the gRPC calls needed by the filesystem service.
pub trait SandboxFilesystemGrpcClient: Send + Sync {
    /// Execute a filesystem operation and return (exec_id, optional file_descriptor).
    fn filesystem_exec(
        &self,
        task_id: &str,
        request: FilesystemExecRequest,
    ) -> Result<FilesystemExecResponse, ModalError>;

    /// Get the output of a filesystem exec operation.
    fn filesystem_exec_get_output(
        &self,
        exec_id: &str,
    ) -> Result<Vec<u8>, ModalError>;
}

/// Filesystem exec request types.
#[derive(Debug, Clone)]
pub enum FilesystemExecRequest {
    Open { path: String, mode: String },
    Read { file_descriptor: String, n: Option<usize> },
    ReadLine { file_descriptor: String },
    Write { file_descriptor: String, data: Vec<u8> },
    Flush { file_descriptor: String },
    Seek { file_descriptor: String, offset: i64, whence: SeekWhence },
    Close { file_descriptor: String },
    Ls { path: String },
    Mkdir { path: String, parents: bool },
    Rm { path: String, recursive: bool },
}

/// Filesystem exec response.
#[derive(Debug, Clone)]
pub struct FilesystemExecResponse {
    pub exec_id: String,
    pub file_descriptor: Option<String>,
}

/// Implementation of SandboxFilesystemService backed by a gRPC client.
pub struct SandboxFilesystemServiceImpl<C: SandboxFilesystemGrpcClient> {
    pub client: C,
}

impl<C: SandboxFilesystemGrpcClient> SandboxFilesystemService for SandboxFilesystemServiceImpl<C> {
    fn open(&self, task_id: &str, path: &str, mode: &str) -> Result<FileIO, ModalError> {
        let file_mode = FileMode::parse(mode)?;

        let resp = self.client.filesystem_exec(
            task_id,
            FilesystemExecRequest::Open {
                path: path.to_string(),
                mode: mode.to_string(),
            },
        )?;

        let file_descriptor = resp.file_descriptor.ok_or_else(|| {
            ModalError::SandboxFilesystem("failed to open file: no file descriptor returned".to_string())
        })?;

        // Wait for the open operation to complete
        self.client.filesystem_exec_get_output(&resp.exec_id)?;

        Ok(FileIO::new(task_id.to_string(), file_descriptor, file_mode))
    }

    fn read(&self, file_io: &FileIO, n: Option<usize>) -> Result<Vec<u8>, ModalError> {
        file_io.check_closed()?;
        file_io.check_readable()?;
        FileIO::validate_read_size(n)?;

        let resp = self.client.filesystem_exec(
            &file_io.task_id,
            FilesystemExecRequest::Read {
                file_descriptor: file_io.file_descriptor.clone(),
                n,
            },
        )?;

        self.client.filesystem_exec_get_output(&resp.exec_id)
    }

    fn readline(&self, file_io: &FileIO) -> Result<Vec<u8>, ModalError> {
        file_io.check_closed()?;
        file_io.check_readable()?;

        let resp = self.client.filesystem_exec(
            &file_io.task_id,
            FilesystemExecRequest::ReadLine {
                file_descriptor: file_io.file_descriptor.clone(),
            },
        )?;

        self.client.filesystem_exec_get_output(&resp.exec_id)
    }

    fn write(&self, file_io: &FileIO, data: &[u8]) -> Result<(), ModalError> {
        file_io.check_closed()?;
        file_io.check_writable()?;
        FileIO::validate_write_size(data)?;

        for chunk in FileIO::write_chunks(data) {
            let resp = self.client.filesystem_exec(
                &file_io.task_id,
                FilesystemExecRequest::Write {
                    file_descriptor: file_io.file_descriptor.clone(),
                    data: chunk.to_vec(),
                },
            )?;
            self.client.filesystem_exec_get_output(&resp.exec_id)?;
        }

        Ok(())
    }

    fn flush(&self, file_io: &FileIO) -> Result<(), ModalError> {
        file_io.check_closed()?;
        file_io.check_writable()?;

        let resp = self.client.filesystem_exec(
            &file_io.task_id,
            FilesystemExecRequest::Flush {
                file_descriptor: file_io.file_descriptor.clone(),
            },
        )?;

        self.client.filesystem_exec_get_output(&resp.exec_id)?;
        Ok(())
    }

    fn seek(&self, file_io: &FileIO, offset: i64, whence: SeekWhence) -> Result<(), ModalError> {
        file_io.check_closed()?;

        let resp = self.client.filesystem_exec(
            &file_io.task_id,
            FilesystemExecRequest::Seek {
                file_descriptor: file_io.file_descriptor.clone(),
                offset,
                whence,
            },
        )?;

        self.client.filesystem_exec_get_output(&resp.exec_id)?;
        Ok(())
    }

    fn close(&self, file_io: &mut FileIO) -> Result<(), ModalError> {
        let resp = self.client.filesystem_exec(
            &file_io.task_id,
            FilesystemExecRequest::Close {
                file_descriptor: file_io.file_descriptor.clone(),
            },
        )?;

        file_io.close();
        self.client.filesystem_exec_get_output(&resp.exec_id)?;
        Ok(())
    }

    fn ls(&self, task_id: &str, path: &str) -> Result<Vec<String>, ModalError> {
        let resp = self.client.filesystem_exec(
            task_id,
            FilesystemExecRequest::Ls {
                path: path.to_string(),
            },
        )?;

        let output = self.client.filesystem_exec_get_output(&resp.exec_id)?;
        let listing = DirListing::from_json(&output)?;
        Ok(listing.paths)
    }

    fn mkdir(&self, task_id: &str, path: &str, parents: bool) -> Result<(), ModalError> {
        let resp = self.client.filesystem_exec(
            task_id,
            FilesystemExecRequest::Mkdir {
                path: path.to_string(),
                parents,
            },
        )?;

        self.client.filesystem_exec_get_output(&resp.exec_id)?;
        Ok(())
    }

    fn rm(&self, task_id: &str, path: &str, recursive: bool) -> Result<(), ModalError> {
        let resp = self.client.filesystem_exec(
            task_id,
            FilesystemExecRequest::Rm {
                path: path.to_string(),
                recursive,
            },
        )?;

        self.client.filesystem_exec_get_output(&resp.exec_id)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // --- FileMode tests ---

    #[test]
    fn test_file_mode_read() {
        let mode = FileMode::parse("r").unwrap();
        assert!(mode.readable);
        assert!(!mode.writable);
        assert!(!mode.appended);
        assert!(!mode.binary);
    }

    #[test]
    fn test_file_mode_write() {
        let mode = FileMode::parse("w").unwrap();
        assert!(!mode.readable);
        assert!(mode.writable);
        assert!(!mode.appended);
        assert!(!mode.binary);
    }

    #[test]
    fn test_file_mode_append() {
        let mode = FileMode::parse("a").unwrap();
        assert!(!mode.readable);
        assert!(mode.writable);
        assert!(mode.appended);
        assert!(!mode.binary);
    }

    #[test]
    fn test_file_mode_exclusive_create() {
        let mode = FileMode::parse("x").unwrap();
        assert!(!mode.readable);
        assert!(mode.writable);
        assert!(!mode.appended);
        assert!(!mode.binary);
    }

    #[test]
    fn test_file_mode_read_binary() {
        let mode = FileMode::parse("rb").unwrap();
        assert!(mode.readable);
        assert!(!mode.writable);
        assert!(mode.binary);
    }

    #[test]
    fn test_file_mode_write_binary() {
        let mode = FileMode::parse("wb").unwrap();
        assert!(!mode.readable);
        assert!(mode.writable);
        assert!(mode.binary);
    }

    #[test]
    fn test_file_mode_read_plus() {
        let mode = FileMode::parse("r+").unwrap();
        assert!(mode.readable);
        assert!(mode.writable);
        assert!(!mode.binary);
    }

    #[test]
    fn test_file_mode_write_binary_plus() {
        let mode = FileMode::parse("wb+").unwrap();
        assert!(mode.readable);
        assert!(mode.writable);
        assert!(mode.binary);
    }

    #[test]
    fn test_file_mode_empty() {
        let err = FileMode::parse("").unwrap_err();
        assert!(err.to_string().contains("empty"));
    }

    #[test]
    fn test_file_mode_invalid_char() {
        let err = FileMode::parse("z").unwrap_err();
        assert!(err.to_string().contains("invalid file mode"));
    }

    #[test]
    fn test_file_mode_duplicate_chars() {
        let err = FileMode::parse("rr").unwrap_err();
        assert!(err.to_string().contains("invalid file mode"));
    }

    #[test]
    fn test_file_mode_multiple_rwax() {
        let err = FileMode::parse("rw").unwrap_err();
        assert!(err.to_string().contains("exactly one"));
    }

    #[test]
    fn test_file_mode_as_str() {
        let mode = FileMode::parse("rb").unwrap();
        assert_eq!(mode.as_str(), "rb");
    }

    // --- SeekWhence tests ---

    #[test]
    fn test_seek_whence_from_i32() {
        assert_eq!(SeekWhence::from_i32(0).unwrap(), SeekWhence::Set);
        assert_eq!(SeekWhence::from_i32(1).unwrap(), SeekWhence::Current);
        assert_eq!(SeekWhence::from_i32(2).unwrap(), SeekWhence::End);
    }

    #[test]
    fn test_seek_whence_invalid() {
        let err = SeekWhence::from_i32(3).unwrap_err();
        assert!(err.to_string().contains("invalid whence value"));
    }

    // --- SystemErrorCode tests ---

    #[test]
    fn test_system_error_code_from_i32() {
        assert_eq!(SystemErrorCode::from_i32(0), SystemErrorCode::Unspecified);
        assert_eq!(SystemErrorCode::from_i32(1), SystemErrorCode::PermissionDenied);
        assert_eq!(SystemErrorCode::from_i32(2), SystemErrorCode::NotFound);
        assert_eq!(SystemErrorCode::from_i32(5), SystemErrorCode::IoError);
        assert_eq!(SystemErrorCode::from_i32(13), SystemErrorCode::AccessDenied);
        assert_eq!(SystemErrorCode::from_i32(17), SystemErrorCode::AlreadyExists);
        assert_eq!(SystemErrorCode::from_i32(21), SystemErrorCode::IsDirectory);
        assert_eq!(SystemErrorCode::from_i32(28), SystemErrorCode::NoSpaceLeft);
        assert_eq!(SystemErrorCode::from_i32(999), SystemErrorCode::Unspecified);
    }

    #[test]
    fn test_system_error_code_to_error() {
        let err = SystemErrorCode::NotFound.to_error("/tmp/missing.txt");
        assert!(matches!(err, ModalError::NotFound(_)));

        let err = SystemErrorCode::AlreadyExists.to_error("/tmp/exists.txt");
        assert!(matches!(err, ModalError::AlreadyExists(_)));

        let err = SystemErrorCode::PermissionDenied.to_error("/root/secret");
        assert!(matches!(err, ModalError::SandboxFilesystem(_)));
        assert!(err.to_string().contains("operation not permitted"));
    }

    // --- FileWatchEventType tests ---

    #[test]
    fn test_file_watch_event_type_from_str() {
        assert_eq!(FileWatchEventType::from_str("Create"), FileWatchEventType::Create);
        assert_eq!(FileWatchEventType::from_str("Modify"), FileWatchEventType::Modify);
        assert_eq!(FileWatchEventType::from_str("Remove"), FileWatchEventType::Remove);
        assert_eq!(FileWatchEventType::from_str("Access"), FileWatchEventType::Access);
        assert_eq!(FileWatchEventType::from_str("bogus"), FileWatchEventType::Unknown);
    }

    #[test]
    fn test_file_watch_event_type_as_str() {
        assert_eq!(FileWatchEventType::Create.as_str(), "Create");
        assert_eq!(FileWatchEventType::Unknown.as_str(), "Unknown");
    }

    // --- FileWatchEvent tests ---

    #[test]
    fn test_file_watch_event_from_json() {
        let json = br#"{"event_type": "Create", "paths": ["/tmp/foo.txt"]}"#;
        let event = FileWatchEvent::from_json(json).unwrap();
        assert_eq!(event.event_type, FileWatchEventType::Create);
        assert_eq!(event.paths, vec!["/tmp/foo.txt"]);
    }

    #[test]
    fn test_file_watch_event_from_json_multiple_paths() {
        let json = br#"{"event_type": "Modify", "paths": ["/a", "/b"]}"#;
        let event = FileWatchEvent::from_json(json).unwrap();
        assert_eq!(event.event_type, FileWatchEventType::Modify);
        assert_eq!(event.paths, vec!["/a", "/b"]);
    }

    #[test]
    fn test_file_watch_event_from_json_invalid() {
        let err = FileWatchEvent::from_json(b"not json").unwrap_err();
        assert!(err.to_string().contains("failed to parse watch event"));
    }

    // --- DirListing tests ---

    #[test]
    fn test_dir_listing_from_json() {
        let json = br#"{"paths": ["a.txt", "b.txt", "subdir"]}"#;
        let listing = DirListing::from_json(json).unwrap();
        assert_eq!(listing.paths, vec!["a.txt", "b.txt", "subdir"]);
    }

    #[test]
    fn test_dir_listing_from_json_empty() {
        let json = br#"{"paths": []}"#;
        let listing = DirListing::from_json(json).unwrap();
        assert!(listing.paths.is_empty());
    }

    #[test]
    fn test_dir_listing_from_json_invalid() {
        let err = DirListing::from_json(b"{}").unwrap_err();
        assert!(err.to_string().contains("failed to parse list output"));
    }

    // --- FileIO validation tests ---

    #[test]
    fn test_file_io_check_closed() {
        let mut fio = FileIO::new("task-1".into(), "fd-1".into(), FileMode::parse("r").unwrap());
        assert!(fio.check_closed().is_ok());
        fio.close();
        assert!(fio.is_closed());
        let err = fio.check_closed().unwrap_err();
        assert!(err.to_string().contains("closed file"));
    }

    #[test]
    fn test_file_io_check_readable() {
        let fio_r = FileIO::new("task-1".into(), "fd-1".into(), FileMode::parse("r").unwrap());
        assert!(fio_r.check_readable().is_ok());

        let fio_w = FileIO::new("task-1".into(), "fd-1".into(), FileMode::parse("w").unwrap());
        let err = fio_w.check_readable().unwrap_err();
        assert!(err.to_string().contains("not readable"));
    }

    #[test]
    fn test_file_io_check_writable() {
        let fio_w = FileIO::new("task-1".into(), "fd-1".into(), FileMode::parse("w").unwrap());
        assert!(fio_w.check_writable().is_ok());

        let fio_r = FileIO::new("task-1".into(), "fd-1".into(), FileMode::parse("r").unwrap());
        let err = fio_r.check_writable().unwrap_err();
        assert!(err.to_string().contains("not writable"));
    }

    #[test]
    fn test_file_io_validate_read_size() {
        assert!(FileIO::validate_read_size(None).is_ok());
        assert!(FileIO::validate_read_size(Some(1024)).is_ok());
        assert!(FileIO::validate_read_size(Some(READ_FILE_SIZE_LIMIT)).is_ok());
        let err = FileIO::validate_read_size(Some(READ_FILE_SIZE_LIMIT + 1)).unwrap_err();
        assert!(err.to_string().contains("100 MiB limit"));
    }

    #[test]
    fn test_file_io_validate_write_size() {
        assert!(FileIO::validate_write_size(&[0u8; 1024]).is_ok());
        let err = FileIO::validate_write_size(&vec![0u8; WRITE_FILE_SIZE_LIMIT + 1]).unwrap_err();
        assert!(err.to_string().contains("1 GiB limit"));
    }

    #[test]
    fn test_file_io_write_chunks() {
        let small_data = vec![0u8; 100];
        let chunks = FileIO::write_chunks(&small_data);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].len(), 100);

        // Exactly one chunk boundary
        let exact_data = vec![0u8; WRITE_CHUNK_SIZE];
        let chunks = FileIO::write_chunks(&exact_data);
        assert_eq!(chunks.len(), 1);

        // Just over one chunk
        let over_data = vec![0u8; WRITE_CHUNK_SIZE + 1];
        let chunks = FileIO::write_chunks(&over_data);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].len(), WRITE_CHUNK_SIZE);
        assert_eq!(chunks[1].len(), 1);
    }

    // --- SandboxFilesystemServiceImpl tests with mock gRPC client ---

    struct MockFilesystemGrpcClient {
        responses: Mutex<Vec<MockFsResponse>>,
    }

    enum MockFsResponse {
        Exec(Result<FilesystemExecResponse, ModalError>),
        Output(Result<Vec<u8>, ModalError>),
    }

    impl MockFilesystemGrpcClient {
        fn new() -> Self {
            Self {
                responses: Mutex::new(Vec::new()),
            }
        }

        fn push_exec(&self, result: Result<FilesystemExecResponse, ModalError>) {
            self.responses.lock().unwrap().push(MockFsResponse::Exec(result));
        }

        fn push_output(&self, result: Result<Vec<u8>, ModalError>) {
            self.responses.lock().unwrap().push(MockFsResponse::Output(result));
        }
    }

    impl SandboxFilesystemGrpcClient for MockFilesystemGrpcClient {
        fn filesystem_exec(
            &self,
            _task_id: &str,
            _request: FilesystemExecRequest,
        ) -> Result<FilesystemExecResponse, ModalError> {
            let mut responses = self.responses.lock().unwrap();
            match responses.remove(0) {
                MockFsResponse::Exec(r) => r,
                _ => panic!("expected Exec mock response"),
            }
        }

        fn filesystem_exec_get_output(
            &self,
            _exec_id: &str,
        ) -> Result<Vec<u8>, ModalError> {
            let mut responses = self.responses.lock().unwrap();
            match responses.remove(0) {
                MockFsResponse::Output(r) => r,
                _ => panic!("expected Output mock response"),
            }
        }
    }

    fn make_service(mock: MockFilesystemGrpcClient) -> SandboxFilesystemServiceImpl<MockFilesystemGrpcClient> {
        SandboxFilesystemServiceImpl { client: mock }
    }

    #[test]
    fn test_open_file() {
        let mock = MockFilesystemGrpcClient::new();
        mock.push_exec(Ok(FilesystemExecResponse {
            exec_id: "exec-1".to_string(),
            file_descriptor: Some("fd-abc".to_string()),
        }));
        mock.push_output(Ok(vec![]));
        let svc = make_service(mock);

        let fio = svc.open("task-1", "/tmp/test.txt", "r").unwrap();
        assert_eq!(fio.file_descriptor, "fd-abc");
        assert_eq!(fio.task_id, "task-1");
        assert!(fio.mode.readable);
        assert!(!fio.mode.writable);
        assert!(!fio.is_closed());
    }

    #[test]
    fn test_open_file_no_descriptor() {
        let mock = MockFilesystemGrpcClient::new();
        mock.push_exec(Ok(FilesystemExecResponse {
            exec_id: "exec-1".to_string(),
            file_descriptor: None,
        }));
        let svc = make_service(mock);

        let err = svc.open("task-1", "/tmp/test.txt", "r").unwrap_err();
        assert!(err.to_string().contains("no file descriptor"));
    }

    #[test]
    fn test_open_file_invalid_mode() {
        let mock = MockFilesystemGrpcClient::new();
        let svc = make_service(mock);

        let err = svc.open("task-1", "/tmp/test.txt", "z").unwrap_err();
        assert!(err.to_string().contains("invalid file mode"));
    }

    #[test]
    fn test_read_file() {
        let mock = MockFilesystemGrpcClient::new();
        mock.push_exec(Ok(FilesystemExecResponse {
            exec_id: "exec-read".to_string(),
            file_descriptor: None,
        }));
        mock.push_output(Ok(b"hello world".to_vec()));
        let svc = make_service(mock);

        let fio = FileIO::new("task-1".into(), "fd-1".into(), FileMode::parse("r").unwrap());
        let data = svc.read(&fio, None).unwrap();
        assert_eq!(data, b"hello world");
    }

    #[test]
    fn test_read_file_with_limit() {
        let mock = MockFilesystemGrpcClient::new();
        mock.push_exec(Ok(FilesystemExecResponse {
            exec_id: "exec-read".to_string(),
            file_descriptor: None,
        }));
        mock.push_output(Ok(b"hello".to_vec()));
        let svc = make_service(mock);

        let fio = FileIO::new("task-1".into(), "fd-1".into(), FileMode::parse("r").unwrap());
        let data = svc.read(&fio, Some(5)).unwrap();
        assert_eq!(data, b"hello");
    }

    #[test]
    fn test_read_closed_file() {
        let mock = MockFilesystemGrpcClient::new();
        let svc = make_service(mock);

        let mut fio = FileIO::new("task-1".into(), "fd-1".into(), FileMode::parse("r").unwrap());
        fio.close();
        let err = svc.read(&fio, None).unwrap_err();
        assert!(err.to_string().contains("closed file"));
    }

    #[test]
    fn test_read_write_only_file() {
        let mock = MockFilesystemGrpcClient::new();
        let svc = make_service(mock);

        let fio = FileIO::new("task-1".into(), "fd-1".into(), FileMode::parse("w").unwrap());
        let err = svc.read(&fio, None).unwrap_err();
        assert!(err.to_string().contains("not readable"));
    }

    #[test]
    fn test_readline() {
        let mock = MockFilesystemGrpcClient::new();
        mock.push_exec(Ok(FilesystemExecResponse {
            exec_id: "exec-readline".to_string(),
            file_descriptor: None,
        }));
        mock.push_output(Ok(b"first line\n".to_vec()));
        let svc = make_service(mock);

        let fio = FileIO::new("task-1".into(), "fd-1".into(), FileMode::parse("r").unwrap());
        let data = svc.readline(&fio).unwrap();
        assert_eq!(data, b"first line\n");
    }

    #[test]
    fn test_write_file() {
        let mock = MockFilesystemGrpcClient::new();
        mock.push_exec(Ok(FilesystemExecResponse {
            exec_id: "exec-write".to_string(),
            file_descriptor: None,
        }));
        mock.push_output(Ok(vec![]));
        let svc = make_service(mock);

        let fio = FileIO::new("task-1".into(), "fd-1".into(), FileMode::parse("w").unwrap());
        svc.write(&fio, b"hello").unwrap();
    }

    #[test]
    fn test_write_read_only_file() {
        let mock = MockFilesystemGrpcClient::new();
        let svc = make_service(mock);

        let fio = FileIO::new("task-1".into(), "fd-1".into(), FileMode::parse("r").unwrap());
        let err = svc.write(&fio, b"hello").unwrap_err();
        assert!(err.to_string().contains("not writable"));
    }

    #[test]
    fn test_flush_file() {
        let mock = MockFilesystemGrpcClient::new();
        mock.push_exec(Ok(FilesystemExecResponse {
            exec_id: "exec-flush".to_string(),
            file_descriptor: None,
        }));
        mock.push_output(Ok(vec![]));
        let svc = make_service(mock);

        let fio = FileIO::new("task-1".into(), "fd-1".into(), FileMode::parse("w").unwrap());
        svc.flush(&fio).unwrap();
    }

    #[test]
    fn test_seek_file() {
        let mock = MockFilesystemGrpcClient::new();
        mock.push_exec(Ok(FilesystemExecResponse {
            exec_id: "exec-seek".to_string(),
            file_descriptor: None,
        }));
        mock.push_output(Ok(vec![]));
        let svc = make_service(mock);

        let fio = FileIO::new("task-1".into(), "fd-1".into(), FileMode::parse("r").unwrap());
        svc.seek(&fio, 10, SeekWhence::Set).unwrap();
    }

    #[test]
    fn test_close_file() {
        let mock = MockFilesystemGrpcClient::new();
        mock.push_exec(Ok(FilesystemExecResponse {
            exec_id: "exec-close".to_string(),
            file_descriptor: None,
        }));
        mock.push_output(Ok(vec![]));
        let svc = make_service(mock);

        let mut fio = FileIO::new("task-1".into(), "fd-1".into(), FileMode::parse("r").unwrap());
        assert!(!fio.is_closed());
        svc.close(&mut fio).unwrap();
        assert!(fio.is_closed());
    }

    #[test]
    fn test_ls_directory() {
        let mock = MockFilesystemGrpcClient::new();
        mock.push_exec(Ok(FilesystemExecResponse {
            exec_id: "exec-ls".to_string(),
            file_descriptor: None,
        }));
        mock.push_output(Ok(br#"{"paths": ["file1.txt", "file2.txt", "subdir"]}"#.to_vec()));
        let svc = make_service(mock);

        let paths = svc.ls("task-1", "/tmp").unwrap();
        assert_eq!(paths, vec!["file1.txt", "file2.txt", "subdir"]);
    }

    #[test]
    fn test_mkdir() {
        let mock = MockFilesystemGrpcClient::new();
        mock.push_exec(Ok(FilesystemExecResponse {
            exec_id: "exec-mkdir".to_string(),
            file_descriptor: None,
        }));
        mock.push_output(Ok(vec![]));
        let svc = make_service(mock);

        svc.mkdir("task-1", "/tmp/newdir", false).unwrap();
    }

    #[test]
    fn test_mkdir_parents() {
        let mock = MockFilesystemGrpcClient::new();
        mock.push_exec(Ok(FilesystemExecResponse {
            exec_id: "exec-mkdir".to_string(),
            file_descriptor: None,
        }));
        mock.push_output(Ok(vec![]));
        let svc = make_service(mock);

        svc.mkdir("task-1", "/tmp/a/b/c", true).unwrap();
    }

    #[test]
    fn test_rm_file() {
        let mock = MockFilesystemGrpcClient::new();
        mock.push_exec(Ok(FilesystemExecResponse {
            exec_id: "exec-rm".to_string(),
            file_descriptor: None,
        }));
        mock.push_output(Ok(vec![]));
        let svc = make_service(mock);

        svc.rm("task-1", "/tmp/old.txt", false).unwrap();
    }

    #[test]
    fn test_rm_recursive() {
        let mock = MockFilesystemGrpcClient::new();
        mock.push_exec(Ok(FilesystemExecResponse {
            exec_id: "exec-rm".to_string(),
            file_descriptor: None,
        }));
        mock.push_output(Ok(vec![]));
        let svc = make_service(mock);

        svc.rm("task-1", "/tmp/dir", true).unwrap();
    }

    #[test]
    fn test_service_grpc_error_propagation() {
        let mock = MockFilesystemGrpcClient::new();
        mock.push_exec(Err(ModalError::Grpc(tonic::Status::unavailable("server down"))));
        let svc = make_service(mock);

        let err = svc.ls("task-1", "/tmp").unwrap_err();
        assert!(matches!(err, ModalError::Grpc(_)));
    }
}
