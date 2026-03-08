/// Image represents a Modal container image.
#[derive(Debug, Clone)]
pub struct Image {
    pub image_id: String,
}

/// ImageService provides Image related operations.
pub trait ImageService: Send + Sync {
    // Service methods will be added for integration tests
}
