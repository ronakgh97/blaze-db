use memmap2::Mmap;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Result;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Ingestor {
    pub source: PathBuf,
    pub batch_size: usize,
}

impl Ingestor {
    pub fn new(source: impl Into<PathBuf>, batch_size: usize) -> Self {
        let source = source.into();
        assert_eq!(batch_size % 8, 0, "Batch size must be a multiple of 8");
        assert!(
            source.exists() && source.is_file(),
            "Source file must exist and be a file"
        );
        Self { source, batch_size }
    }

    /// Read lines from the source file and batch them (DEPRECATED - use read_chunks for better semantic search)
    pub fn read_line(&self) -> Result<Vec<Vec<String>>> {
        let file = File::open(&self.source)?;
        let mmap = unsafe { Mmap::map(&file)? };

        let lines: Vec<String> = mmap
            .par_split(|&b| b == b'\n')
            .filter_map(|line_bytes| {
                if line_bytes.is_empty() {
                    return None;
                }
                // Decode each line as UTF-8, ignoring invalid sequences
                let s = String::from_utf8_lossy(line_bytes).trim().to_string();
                if s.is_empty() { None } else { Some(s) }
            })
            .collect();

        Ok(lines.into_par_iter().chunks(self.batch_size).collect())
    }

    /// Read file content and split into chunks with overlap
    ///
    /// This method creates better chunks for embedding by:
    /// Respecting sentence/paragraph boundaries
    /// Adding overlap between chunks for context continuity
    ///
    /// * `chunk_size` - Target number of words per chunk
    /// * `overlap` - Number of words to overlap between chunks
    pub fn read_chunks(&self, chunk_size: usize, overlap: usize) -> Result<Vec<Vec<String>>> {
        let file = File::open(&self.source)?;
        let mmap = unsafe { Mmap::map(&file)? };
        let content = String::from_utf8_lossy(&mmap);

        // Normalize line endings and get all text
        let normalized = content.replace("\r\n", "\n");

        // Split into lines, preserving paragraph structure
        let lines: Vec<&str> = normalized.lines().map(|l| l.trim()).collect();

        let mut chunks = Vec::new();
        let mut current_chunk_words: Vec<String> = Vec::new();

        for line in lines {
            if line.is_empty() {
                continue; // Skip empty lines but continue processing
            }

            let line_words: Vec<String> = line.split_whitespace().map(|s| s.to_string()).collect();

            // Check if adding this line would exceed chunk size
            if !current_chunk_words.is_empty()
                && current_chunk_words.len() + line_words.len() > chunk_size
            {
                // Finalize current chunk
                chunks.push(current_chunk_words.join(" "));

                // Prepare overlap for next chunk
                let overlap_start = current_chunk_words.len().saturating_sub(overlap);
                let overlap_buffer = current_chunk_words[overlap_start..].to_vec();

                // Start new chunk with overlap
                current_chunk_words = overlap_buffer;
            }

            // Add words from current line
            current_chunk_words.extend(line_words);
        }

        // Add final chunk if there's remaining content
        if !current_chunk_words.is_empty() {
            chunks.push(current_chunk_words.join(" "));
        }

        // Batch the chunks
        Ok(chunks.into_par_iter().chunks(self.batch_size).collect())
    }
}
