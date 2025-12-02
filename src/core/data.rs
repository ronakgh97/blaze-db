use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize)]
pub struct Source {
    pub path: Option<Vec<PathBuf>>,
}

impl Default for Source {
    fn default() -> Self {
        Self { path: None }
    }
}

impl Source {
    pub async fn add_source_path(mut self, new_path: PathBuf) -> Self {
        match &mut self.path {
            Some(paths) => paths.push(new_path),
            None => self.path = Some(vec![new_path]),
        }
        self
    }

    pub async fn create_source_dir(&self) -> Result<()> {
        if let Some(paths) = &self.path {
            for path in paths {
                if !path.exists() {
                    std::fs::create_dir_all(path)?;
                }
            }
        }
        Ok(())
    }
}
