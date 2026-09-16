use std::fmt;

#[derive(Debug)]
pub struct GitError {
    pub message: String,
    pub stderr: Option<String>,
}

impl GitError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            stderr: None,
        }
    }

    pub fn with_stderr(message: impl Into<String>, stderr: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            stderr: Some(stderr.into()),
        }
    }
}

impl fmt::Display for GitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)?;
        if let Some(stderr) = self
            .stderr
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            write!(f, ": {}", stderr.trim_end())?;
        }
        Ok(())
    }
}

impl std::error::Error for GitError {}

impl From<std::io::Error> for GitError {
    fn from(err: std::io::Error) -> Self {
        GitError::new(format!("failed to run git: {err}"))
    }
}

impl From<std::string::FromUtf8Error> for GitError {
    fn from(err: std::string::FromUtf8Error) -> Self {
        GitError::new(format!("git output is not valid utf-8: {err}"))
    }
}

pub type Result<T> = std::result::Result<T, GitError>;
