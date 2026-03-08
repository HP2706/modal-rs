/// SandboxFile represents a file in a sandbox filesystem.
#[derive(Debug)]
pub struct SandboxFile {
    pub task_id: String,
    pub path: String,
}
