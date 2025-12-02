use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::fs;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Source {
    pub source_name: Option<Vec<String>>,
}

impl Default for Source {
    fn default() -> Self {
        Self { source_name: None }
    }
}

impl Source {
    pub fn add_source(&mut self, new_source: String) -> &mut Self {
        match &mut self.source_name {
            Some(source) => source.push(new_source),
            None => self.source_name = Some(vec![new_source]),
        }
        self
    }

    pub async fn create_source_dir(&self) -> Result<()> {
        if let Some(sources) = &self.source_name {
            for source in sources {
                let path_buf = get_source_path()?.join(source);
                fs::create_dir_all(&path_buf).await?;
            }
        }
        Ok(())
    }
}

pub fn get_source_path() -> Result<PathBuf> {
    let home_dir =
        dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;
    let source_path = home_dir.join(".blaze").join("sources");
    Ok(source_path)
}
