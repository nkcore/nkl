use thiserror::Error;

#[derive(Error, Debug)]
pub enum NKLError {
    #[error(
        "route conflict: \"{hostname}{path_prefix}\" is already registered by PID {pid}. Use --force to override."
    )]
    RouteConflict {
        hostname: String,
        path_prefix: String,
        pid: u32,
    },

    #[error("failed to acquire route lock")]
    LockFailed,

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),
}
