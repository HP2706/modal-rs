/// Volume represents a Modal persistent volume.
#[derive(Debug, Clone)]
pub struct Volume {
    pub volume_id: String,
    read_only: bool,
}

impl Volume {
    pub fn new(volume_id: String) -> Self {
        Self {
            volume_id,
            read_only: false,
        }
    }

    pub fn read_only(&self) -> Self {
        Self {
            volume_id: self.volume_id.clone(),
            read_only: true,
        }
    }

    pub fn is_read_only(&self) -> bool {
        self.read_only
    }
}

/// VolumeService provides Volume related operations.
pub trait VolumeService: Send + Sync {
    // Service methods will be added for integration tests
}
