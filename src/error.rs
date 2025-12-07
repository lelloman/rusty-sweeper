use std::path::PathBuf;
use thiserror::Error;

/// Core library errors
#[derive(Error, Debug)]
pub enum SweeperError {
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),

    #[error("IO error at path '{path}': {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("IO error: {0}")]
    IoSimple(#[from] std::io::Error),

    #[error("Permission denied: {0}")]
    PermissionDenied(PathBuf),

    #[error("Path not found: {0}")]
    PathNotFound(PathBuf),

    #[error("Not a directory: {0}")]
    NotADirectory(PathBuf),

    #[error("Invalid path: {0}")]
    InvalidPath(String),

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("System call failed: {0}")]
    Nix(#[from] nix::Error),

    #[error("{0}")]
    Other(String),
}

impl SweeperError {
    /// Get exit code for this error type
    pub fn exit_code(&self) -> i32 {
        match self {
            SweeperError::Config(_) => 2,
            SweeperError::Io { .. } => 1,
            SweeperError::IoSimple(_) => 1,
            SweeperError::PermissionDenied(_) => 3,
            SweeperError::PathNotFound(_) => 1,
            SweeperError::NotADirectory(_) => 1,
            SweeperError::InvalidPath(_) => 1,
            SweeperError::Json(_) => 1,
            SweeperError::Nix(_) => 1,
            SweeperError::Other(_) => 1,
        }
    }
}

/// Configuration-specific errors
#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Failed to read config file '{path}': {source}")]
    ReadError {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to parse config file '{path}': {source}")]
    ParseError {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },

    #[error("Invalid configuration: {0}")]
    Invalid(String),
}

/// Result type alias for convenience
pub type Result<T> = std::result::Result<T, SweeperError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_messages() {
        let err = ConfigError::Invalid("threshold must be 0-100".into());
        assert!(err.to_string().contains("threshold"));
    }

    #[test]
    fn error_conversion() {
        let config_err = ConfigError::Invalid("test".into());
        let sweeper_err: SweeperError = config_err.into();
        assert!(matches!(sweeper_err, SweeperError::Config(_)));
    }

    #[test]
    fn exit_code_config_error() {
        let err = SweeperError::Config(ConfigError::Invalid("test".into()));
        assert_eq!(err.exit_code(), 2);
    }

    #[test]
    fn exit_code_permission_denied() {
        let err = SweeperError::PermissionDenied(PathBuf::from("/secret"));
        assert_eq!(err.exit_code(), 3);
    }

    #[test]
    fn exit_code_path_not_found() {
        let err = SweeperError::PathNotFound(PathBuf::from("/nonexistent"));
        assert_eq!(err.exit_code(), 1);
    }

    #[test]
    fn exit_code_not_a_directory() {
        let err = SweeperError::NotADirectory(PathBuf::from("/file.txt"));
        assert_eq!(err.exit_code(), 1);
    }

    #[test]
    fn not_a_directory_error_message() {
        let err = SweeperError::NotADirectory(PathBuf::from("/some/file"));
        assert!(err.to_string().contains("Not a directory"));
        assert!(err.to_string().contains("/some/file"));
    }

    #[test]
    fn other_error_message() {
        let err = SweeperError::Other("something went wrong".into());
        assert_eq!(err.to_string(), "something went wrong");
        assert_eq!(err.exit_code(), 1);
    }
}
