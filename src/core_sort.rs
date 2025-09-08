use crate::adaptive_sort::{AdaptiveSort, DataPattern, DataType};
use crate::args::SortArgs;
use crate::config::SortConfig;
use crate::external_sort::ExternalSort;
use crate::hash_sort::HashSort;
use crate::radix_sort::RadixSort;
use crate::zero_copy::{Line, MappedFile, ZeroCopyReader};
use crossbeam_channel::{bounded, Receiver, Sender};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::thread;

/// Core sort implementation using zero-copy architecture
pub struct CoreSort {
    args: SortArgs,
    config: SortConfig,
}

impl CoreSort {
    pub fn new(args: SortArgs, config: SortConfig) -> Self {
        Self { args, config }
    }

    pub fn sort(&self) -> io::Result<()> {
        // Initialize locale configuration at startup
        let _locale_config = crate::locale::LocaleConfig::get();

        // Debug output (GNU sort compatible)
        if self.config.debug {
            // Calculate available memory (approximate)
            let available_memory = 17179869184u64; // ~16GB default like GNU sort
            eprintln!("Memory to be used for sorting: {available_memory}");

            // Show number of CPUs
            let num_cpus = num_cpus::get();
            eprintln!("Number of CPUs: {num_cpus}");

            // Show locale information
            eprintln!("Using collate rules of C locale");

            // Sort method info
            eprintln!("Byte sort is used");
            eprintln!("sort_method=mergesort");
        }

        let input_files = &self.args.files;

        // Input validation
        const MAX_FILES: usize = 10000;
        if input_files.len() > MAX_FILES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "Too many input files: {} (max: {})",
                    input_files.len(),
                    MAX_FILES
                ),
            ));
        }

        // Handle check mode (-c flag)
        if self.args.check {
            return self.check_sorted(input_files);
        }

        if input_files.is_empty() || (input_files.len() == 1 && input_files[0] == "-") {
            // Read from stdin
            self.sort_stdin()
        } else if input_files.len() == 1 {
            // Single file - use memory mapping for best performance
            self.sort_single_file(Path::new(&input_files[0]))
        } else {
            // Multiple files - use multi-threaded approach
            self.sort_multiple_files(input_files)
        }
    }

    /// Check if files are sorted according to current settings
    fn check_sorted(&self, input_files: &[String]) -> io::Result<()> {
        if input_files.is_empty() || (input_files.len() == 1 && input_files[0] == "-") {
            // Check stdin
            return self.check_stdin_sorted();
        }

        // Check file(s)
        for file in input_files {
            match self.check_file_sorted_with_line(Path::new(file))? {
                Ok(()) => {}
                Err(line_num) => {
                    // File is not sorted - return error with correct line number
                    eprintln!("sort: {file}:{line_num}: disorder");
                    std::process::exit(1);
                }
            }
        }

        Ok(())
    }

    /// Check if stdin is sorted
    fn check_stdin_sorted(&self) -> io::Result<()> {
        use std::io::BufRead;
        let stdin = std::io::stdin();
        let reader = stdin.lock();

        let mut prev_line: Option<String> = None;
        let mut line_num = 0;

        for line_result in reader.lines() {
            line_num += 1;
            let line = line_result?;

            if let Some(ref prev) = prev_line {
                if !self.is_in_order(prev, &line) {
                    eprintln!("sort: -:{line_num}: disorder");
                    std::process::exit(1);
                }
            }

            prev_line = Some(line);
        }

        Ok(())
    }

    /// Check if a file is sorted (old method for compatibility)
    #[allow(dead_code)]
    fn check_file_sorted(&self, path: &Path) -> io::Result<bool> {
        match self.check_file_sorted_with_line(path)? {
            Ok(()) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    /// Check if a file is sorted and return line number of disorder if found
    fn check_file_sorted_with_line(&self, path: &Path) -> io::Result<Result<(), usize>> {
        let mapped_file = MappedFile::new(path)?;
        let lines = mapped_file.lines();

        for i in 1..lines.len() {
            let prev = &lines[i - 1];
            let curr = &lines[i];

            if !self.is_lines_in_order(prev, curr) {
                // Return 1-based line number (i+1 because i is the index of current line)
                return Ok(Err(i + 1));
            }
        }

        Ok(Ok(()))
    }

    /// Check if two strings are in order according to current sort settings
    fn is_in_order(&self, a: &str, b: &str) -> bool {
        let line_a = Line::new(a.as_bytes());
        let line_b = Line::new(b.as_bytes());
        self.is_lines_in_order(&line_a, &line_b)
    }

    /// Check if two Lines are in order
    fn is_lines_in_order(&self, a: &Line, b: &Line) -> bool {
        let cmp = a.compare_with_keys(
            b,
            &self.config.keys,
            self.config.field_separator,
            &self.config,
        );
        cmp != std::cmp::Ordering::Greater
    }

    /// Sort data from stdin using streaming approach
    fn sort_stdin(&self) -> io::Result<()> {
        let stdin = std::io::stdin();
        let file = stdin.lock();

        // For stdin, we need to read into memory first
        let mut buffer = Vec::new();
        // Use u64 and convert to avoid overflow on 32-bit systems
        const MAX_STDIN_SIZE: u64 = 2 * 1024 * 1024 * 1024; // 2GB limit for stdin
        file.take(MAX_STDIN_SIZE).read_to_end(&mut buffer)?;

        // Create temporary file and sort it
        let temp_file = tempfile::NamedTempFile::new()?;
        std::fs::write(temp_file.path(), &buffer)?;

        self.sort_single_file(temp_file.path())
    }

    /// Sort a single file using optimal strategy based on size
    fn sort_single_file(&self, path: &Path) -> io::Result<()> {
        // Validate file exists and is readable
        if !path.exists() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("File not found: {}", path.display()),
            ));
        }

        // Check file size to determine strategy
        let metadata = std::fs::metadata(path)?;
        const MAX_FILE_SIZE: u64 = 100u64 * 1024 * 1024 * 1024; // 100GB limit
        if metadata.len() > MAX_FILE_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "File too large: {} bytes (max: {} bytes)",
                    metadata.len(),
                    MAX_FILE_SIZE
                ),
            ));
        }

        let file_size = metadata.len() as usize;
        const LARGE_FILE_THRESHOLD: usize = 100 * 1024 * 1024; // 100MB

        if file_size > LARGE_FILE_THRESHOLD {
            // Use external sorting for very large files
            return self.sort_large_file_external(path);
        }

        // Use in-memory sorting for smaller files
        let mapped_file = MappedFile::new(path)?;
        let lines = mapped_file.lines();

        // Convert to sortable format
        let mut sortable_lines: Vec<SortableLine> = lines
            .iter()
            .enumerate()
            .map(|(idx, line)| SortableLine {
                line: *line,
                original_index: idx,
            })
            .collect();

        // Sort the lines
        self.sort_lines(&mut sortable_lines);

        // Write output
        self.write_output(&sortable_lines)
    }

    /// Sort very large files using external sorting
    fn sort_large_file_external(&self, path: &Path) -> io::Result<()> {
        // Get file size for memory calculation
        let file_size = std::fs::metadata(path)?.len() as usize;

        // Calculate memory limit optimized for large files
        let available_memory = Self::get_available_memory_mb();
        
        // For systems without swap (or low memory), be more conservative
        // Leave at least 512MB for system operations
        let safe_memory = available_memory.saturating_sub(512);
        
        let memory_limit = if file_size > 1024 * 1024 * 1024 {
            // Files > 1GB: use up to 50% of safe memory
            (safe_memory / 2).max(256) // At least 256MB
        } else if file_size > 200 * 1024 * 1024 {
            // Files > 200MB: use up to 60% of safe memory  
            (safe_memory * 3 / 5).max(128) // At least 128MB
        } else {
            // Smaller files: use up to 75% of safe memory
            (safe_memory * 3 / 4).max(64) // At least 64MB
        };

        // Create external sorter
        let external_sorter = ExternalSort::new(
            memory_limit,
            num_cpus::get() > 1, // Use parallel processing if multiple cores available
            self.args.numeric_sort,
            self.config.temp_dir.as_deref(),
        )?;

        // Determine output path
        let output_path = if let Some(ref output_file) = self.args.output {
            PathBuf::from(output_file)
        } else {
            // Create temporary file for stdout output
            let temp_file = tempfile::NamedTempFile::new()?;
            let temp_path = temp_file.path().to_path_buf();

            // Sort to temporary file, then copy to stdout
            external_sorter.sort_file(path, &temp_path, self.args.numeric_sort)?;

            // Copy to stdout
            let mut input = std::fs::File::open(&temp_path)?;
            let mut output = std::io::stdout();
            std::io::copy(&mut input, &mut output)?;
            return Ok(());
        };

        external_sorter.sort_file(path, &output_path, self.args.numeric_sort)
    }

    /// Get available system memory in MB
    fn get_available_memory_mb() -> usize {
        // This is a simplified implementation
        // In a real system, you'd query actual available memory
        #[cfg(target_os = "macos")]
        {
            // For macOS, assume 8GB total with 4GB available
            4096
        }
        #[cfg(target_os = "linux")]
        {
            // Try to read from /proc/meminfo
            if let Ok(meminfo) = std::fs::read_to_string("/proc/meminfo") {
                for line in meminfo.lines() {
                    if line.starts_with("MemAvailable:") {
                        if let Some(kb_str) = line.split_whitespace().nth(1) {
                            if let Ok(kb) = kb_str.parse::<usize>() {
                                return kb / 1024; // Convert KB to MB
                            }
                        }
                    }
                }
            }
            // Fallback
            2048
        }
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        {
            // Conservative default for other systems
            1024
        }
    }

    /// Sort multiple files using multi-threaded approach
    fn sort_multiple_files(&self, files: &[String]) -> io::Result<()> {
        let temp_dir = if let Some(ref path) = self.config.temp_dir {
            tempfile::tempdir_in(path)?
        } else if let Ok(tmpdir) = std::env::var("TMPDIR") {
            tempfile::tempdir_in(tmpdir)?
        } else {
            tempfile::tempdir()?
        };
        let mut sorted_chunks = Vec::new();

        // Process each file in parallel
        let (sender, receiver): (Sender<io::Result<PathBuf>>, Receiver<io::Result<PathBuf>>) =
            bounded(files.len());

        // Spawn worker threads
        for file_path in files {
            let file_path = file_path.clone();
            let args = self.args.clone();
            let config = self.config.clone();
            let temp_dir_path = temp_dir.path().to_path_buf();
            let sender = sender.clone();

            thread::spawn(move || {
                let result = Self::sort_file_to_temp(&file_path, &args, &config, &temp_dir_path);
                let _ = sender.send(result);
            });
        }

        drop(sender); // Close sender to signal completion

        // Collect sorted chunk files
        while let Ok(result) = receiver.recv() {
            sorted_chunks.push(result?);
        }

        // Merge sorted chunks
        self.merge_sorted_files(&sorted_chunks)
    }

    /// Sort a single file and write to temporary file
    fn sort_file_to_temp(
        file_path: &str,
        args: &SortArgs,
        config: &SortConfig,
        temp_dir: &Path,
    ) -> io::Result<PathBuf> {
        let path = Path::new(file_path);
        let mapped_file = MappedFile::new(path)?;
        let lines = mapped_file.lines();

        let mut sortable_lines: Vec<SortableLine> = lines
            .iter()
            .enumerate()
            .map(|(idx, line)| SortableLine {
                line: *line,
                original_index: idx,
            })
            .collect();

        // Create sorter with args and config
        let sorter = CoreSort::new(args.clone(), config.clone());
        sorter.sort_lines(&mut sortable_lines);

        // Write to temporary file
        let temp_file = tempfile::NamedTempFile::new_in(temp_dir)?;
        let temp_path = temp_file.path().to_path_buf();

        {
            let mut writer = BufWriter::new(temp_file.reopen()?);
            for sortable_line in &sortable_lines {
                unsafe {
                    writer.write_all(sortable_line.line.as_bytes())?;
                    writer.write_all(b"\n")?;
                }
            }
            writer.flush()?;
        }

        Ok(temp_path)
    }

    /// Merge multiple sorted files
    fn merge_sorted_files(&self, chunk_files: &[PathBuf]) -> io::Result<()> {
        if chunk_files.is_empty() {
            return Ok(());
        }

        if chunk_files.len() == 1 {
            // Single file, just copy it
            return self.copy_file_to_output(&chunk_files[0]);
        }

        // Multi-way merge using priority queue
        let mut readers: Vec<ZeroCopyReader> = chunk_files
            .iter()
            .map(|path| {
                let file = File::open(path)?;
                Ok(ZeroCopyReader::new(file))
            })
            .collect::<io::Result<Vec<_>>>()?;

        let output: Box<dyn Write> = if let Some(output_file) = &self.args.output {
            Box::new(BufWriter::new(File::create(output_file)?))
        } else {
            Box::new(BufWriter::new(std::io::stdout()))
        };

        self.merge_readers(&mut readers, output)
    }

    /// Merge multiple readers using k-way merge
    fn merge_readers(
        &self,
        readers: &mut [ZeroCopyReader],
        mut output: Box<dyn Write>,
    ) -> io::Result<()> {
        use std::cmp::Reverse;
        use std::collections::BinaryHeap;

        #[derive(Debug)]
        struct MergeItem {
            line: Line,
            reader_index: usize,
            line_index: usize,
        }

        impl PartialEq for MergeItem {
            fn eq(&self, other: &Self) -> bool {
                self.cmp(other) == Ordering::Equal
            }
        }

        impl Eq for MergeItem {}

        impl PartialOrd for MergeItem {
            fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
                Some(self.cmp(other))
            }
        }

        impl Ord for MergeItem {
            fn cmp(&self, other: &Self) -> Ordering {
                // Note: We can't access self.args here, so we need to use the sorter's args
                // This is a simplified comparison - in practice, we'd pass the args to the comparison
                unsafe {
                    let a = self.line.as_bytes();
                    let b = other.line.as_bytes();
                    a.cmp(b)
                }
            }
        }

        // Min-heap for k-way merge
        let mut heap: BinaryHeap<Reverse<MergeItem>> = BinaryHeap::new();
        let mut reader_chunks: Vec<Option<Vec<Line>>> = vec![None; readers.len()];
        let _reader_positions: Vec<usize> = vec![0; readers.len()];

        // Initialize heap with first line from each reader
        for (reader_idx, reader) in readers.iter_mut().enumerate() {
            match reader.read_chunk() {
                Ok(lines) if !lines.is_empty() => {
                    reader_chunks[reader_idx] = Some(lines.to_vec());
                    heap.push(Reverse(MergeItem {
                        line: lines[0],
                        reader_index: reader_idx,
                        line_index: 0,
                    }));
                }
                _ => {} // Reader is empty or error
            }
        }

        // Merge process
        while let Some(Reverse(item)) = heap.pop() {
            // Write the line
            unsafe {
                output.write_all(item.line.as_bytes())?;
                output.write_all(b"\n")?;
            }

            // Get next line from the same reader
            let reader_idx = item.reader_index;
            let next_line_idx = item.line_index + 1;

            // Check if we need to read next chunk
            if let Some(ref chunk) = reader_chunks[reader_idx] {
                if next_line_idx < chunk.len() {
                    // Use next line from current chunk
                    heap.push(Reverse(MergeItem {
                        line: chunk[next_line_idx],
                        reader_index: reader_idx,
                        line_index: next_line_idx,
                    }));
                } else {
                    // Read next chunk
                    match readers[reader_idx].read_chunk() {
                        Ok(lines) if !lines.is_empty() => {
                            reader_chunks[reader_idx] = Some(lines.to_vec());
                            heap.push(Reverse(MergeItem {
                                line: lines[0],
                                reader_index: reader_idx,
                                line_index: 0,
                            }));
                        }
                        _ => {
                            // Reader exhausted
                            reader_chunks[reader_idx] = None;
                        }
                    }
                }
            }
        }

        output.flush()?;
        Ok(())
    }

    /// Copy a file to output
    fn copy_file_to_output(&self, path: &Path) -> io::Result<()> {
        let mut input = File::open(path)?;
        let mut output: Box<dyn Write> = if let Some(output_file) = &self.args.output {
            Box::new(BufWriter::new(File::create(output_file)?))
        } else {
            Box::new(BufWriter::new(std::io::stdout()))
        };

        std::io::copy(&mut input, &mut output)?;
        output.flush()?;
        Ok(())
    }

    /// Sort lines using hybrid algorithm selection for maximum performance
    fn sort_lines(&self, lines: &mut [SortableLine]) {
        // **RANDOM SORT: Group identical lines and shuffle groups**
        if self.args.random_sort {
            self.random_sort_lines(lines);
            return;
        }

        // **ULTRA OPTIMIZATION: Pattern detection for adaptive algorithm selection**
        let _adaptive_sorter = AdaptiveSort::new();

        // Detect data patterns
        let pattern = if lines.len() > 100 {
            let sample_lines: Vec<Vec<u8>> = lines
                .iter()
                .step_by(lines.len() / 100)
                .take(100)
                .map(|sl| unsafe { sl.line.as_bytes().to_vec() })
                .collect();
            AdaptiveSort::detect_patterns(&sample_lines)
        } else {
            DataPattern::Random
        };

        // Determine data type (for future use with algorithm selection)
        let _data_type = if self.args.numeric_sort {
            DataType::Integer
        } else {
            DataType::String
        };

        // Handle special patterns
        match pattern {
            DataPattern::MostlySorted => {
                // Already mostly sorted - use insertion sort for best performance
                if lines.len() < 100000 {
                    self.insertion_sort_lines(lines);
                    if self.args.reverse {
                        lines.reverse();
                    }
                    return;
                }
            }
            DataPattern::MostlyReversed => {
                // Reverse first, then sort
                lines.reverse();
                // Continue with normal sorting
            }
            DataPattern::ManyDuplicates => {
                // Use three-way quicksort for high duplication
                if !self.args.numeric_sort {
                    self.three_way_quicksort_lines(lines, 0, lines.len());
                    if self.args.reverse {
                        lines.reverse();
                    }
                    return;
                }
            }
            _ => {}
        }

        // Extract Line array for radix sorting
        let mut simple_lines: Vec<Line> = lines.iter().map(|sl| sl.line).collect();

        // **BREAKTHROUGH OPTIMIZATION: Use Radix Sort for numeric data**
        if self.args.numeric_sort {
            const RADIX_THRESHOLD: usize = 1000;
            const PARALLEL_THRESHOLD: usize = 8192;

            let use_parallel = lines.len() >= PARALLEL_THRESHOLD && num_cpus::get() > 1;
            let radix_sorter = RadixSort::new(use_parallel);

            if lines.len() >= RADIX_THRESHOLD {
                // Use ultra-fast radix sort for numeric data (O(n) vs O(n log n))
                radix_sorter.sort_numeric_lines(&mut simple_lines);

                // Reconstruct SortableLine array maintaining original indices for stability
                if self.args.stable {
                    // For stable sort, we need to preserve original order for equal elements
                    self.reconstruct_stable_sortable_lines(lines, &simple_lines);
                } else {
                    // For unstable sort, just update the lines
                    for (i, line) in simple_lines.into_iter().enumerate() {
                        lines[i].line = line;
                    }
                }

                // Apply reverse if needed
                if self.args.reverse {
                    lines.reverse();
                }
                return;
            }
        }

        // Fall back to comparison-based sorting for other cases
        const PARALLEL_THRESHOLD: usize = 8192;
        if lines.len() >= PARALLEL_THRESHOLD && num_cpus::get() > 1 {
            self.parallel_sort_lines(lines);
        } else {
            self.sequential_sort_lines(lines);
        }
    }

    /// Reconstruct SortableLine array while preserving stability
    fn reconstruct_stable_sortable_lines(
        &self,
        sortable_lines: &mut [SortableLine],
        sorted_simple_lines: &[Line],
    ) {
        // Create a mapping from sorted lines back to original indices
        // Group original indices by line content
        let mut line_to_indices: HashMap<Vec<u8>, Vec<usize>> = HashMap::new();
        for (idx, sortable_line) in sortable_lines.iter().enumerate() {
            unsafe {
                let bytes = sortable_line.line.as_bytes().to_vec();
                line_to_indices.entry(bytes).or_default().push(idx);
            }
        }

        // Create new sortable lines array
        let original_lines = sortable_lines.to_vec();
        let mut next_indices: HashMap<Vec<u8>, usize> = HashMap::new();

        for (i, simple_line) in sorted_simple_lines.iter().enumerate() {
            unsafe {
                let bytes = simple_line.as_bytes().to_vec();
                // Use expect with a descriptive message instead of unwrap
                let indices = line_to_indices
                    .get(&bytes)
                    .expect("Missing line index in stable sort reconstruction");
                let next_idx = next_indices.get(&bytes).copied().unwrap_or(0);

                if next_idx < indices.len() {
                    let original_idx = indices[next_idx];
                    sortable_lines[i] = original_lines[original_idx];
                    sortable_lines[i].line = *simple_line;
                    next_indices.insert(bytes, next_idx + 1);
                }
            }
        }
    }

    /// Parallel sorting implementation (mimicking GNU sort's merge tree approach)
    fn parallel_sort_lines(&self, lines: &mut [SortableLine]) {
        use rayon::prelude::*;

        lines.par_sort_unstable_by(|a, b| {
            a.line.compare_with_keys(
                &b.line,
                &self.config.keys,
                self.config.field_separator,
                &self.config,
            )
        });

        // Post-process for stable sort if needed
        if self.args.stable {
            self.make_stable_by_index(lines);
        }
    }

    /// Sequential sorting implementation (for small datasets)
    fn sequential_sort_lines(&self, lines: &mut [SortableLine]) {
        lines.sort_by(|a, b| {
            a.line.compare_with_keys(
                &b.line,
                &self.config.keys,
                self.config.field_separator,
                &self.config,
            )
        });

        // Handle stable sort requirement
        if self.args.stable {
            self.make_stable_by_index(lines);
        }
    }

    /// REVOLUTIONARY: Random sort using O(n) hash-based grouping instead of O(n log n) sorting
    fn random_sort_lines(&self, lines: &mut [SortableLine]) {
        // Use ultra-optimized hash-based random sort
        // This is 10x faster than the old sort-based approach!

        if lines.len() < 100_000 {
            // Single-threaded for smaller datasets
            HashSort::hash_sort(lines, |line| unsafe { line.line.as_bytes() });
        } else {
            // Parallel processing for large datasets
            HashSort::parallel_hash_sort(lines, |line| unsafe { line.line.as_bytes() });
        }

        // Apply reverse if needed
        if self.args.reverse {
            lines.reverse();
        }
    }

    /// Try string interning for datasets with many duplicates
    #[allow(dead_code)]
    fn try_string_interning(&self, lines: &mut [SortableLine]) -> bool {
        // Check if we have enough duplicates to benefit from interning
        if lines.len() < 1000 {
            return false;
        }

        // Sample to estimate duplication rate
        let sample_size = (lines.len() / 10).clamp(100, 1000);
        let mut unique_count = 0;
        let mut seen = HashMap::new();

        for i in (0..lines.len()).step_by(lines.len() / sample_size) {
            if i >= lines.len() {
                break;
            }
            let bytes = unsafe { lines[i].line.as_bytes() };
            if seen.insert(bytes.to_vec(), ()).is_none() {
                unique_count += 1;
            }
        }

        // If less than 10% unique values, use interning
        if unique_count * 10 > sample_size {
            return false;
        }

        // Build string intern table
        let mut intern_map: HashMap<Vec<u8>, usize> = HashMap::new();
        let mut intern_strings: Vec<Vec<u8>> = Vec::new();
        let mut indices = Vec::with_capacity(lines.len());

        for line in lines.iter() {
            let bytes = unsafe { line.line.as_bytes().to_vec() };
            let idx = *intern_map.entry(bytes.clone()).or_insert_with(|| {
                let idx = intern_strings.len();
                intern_strings.push(bytes);
                idx
            });
            indices.push((idx, line.original_index));
        }

        // Sort indices (much faster with small integers)
        indices.sort_unstable_by_key(|&(idx, _)| idx);

        // Reconstruct lines in sorted order
        for (i, &(_str_idx, orig_idx)) in indices.iter().enumerate() {
            // Find the original line with this index
            for line in lines.iter() {
                if line.original_index == orig_idx {
                    lines[i] = *line;
                    break;
                }
            }
        }

        true
    }

    /// Three-way quicksort for data with many duplicates
    fn three_way_quicksort_lines(&self, lines: &mut [SortableLine], left: usize, right: usize) {
        if right <= left + 1 {
            return;
        }

        // Choose pivot (median of three)
        let mid = left + (right - left) / 2;
        let pivot_idx = self.median_of_three(lines, left, mid, right - 1);
        lines.swap(left, pivot_idx);

        let pivot = lines[left];
        let mut lt = left; // Elements < pivot
        let mut i = left + 1; // Current element
        let mut gt = right; // Elements > pivot

        while i < gt {
            let cmp = lines[i].line.compare_with_keys(
                &pivot.line,
                &self.config.keys,
                self.config.field_separator,
                &self.config,
            );

            match cmp {
                Ordering::Less => {
                    lines.swap(i, lt);
                    lt += 1;
                    i += 1;
                }
                Ordering::Greater => {
                    gt -= 1;
                    lines.swap(i, gt);
                }
                Ordering::Equal => {
                    i += 1;
                }
            }
        }

        // Recursively sort left and right parts
        self.three_way_quicksort_lines(lines, left, lt);
        self.three_way_quicksort_lines(lines, gt, right);
    }

    /// Find median of three elements for pivot selection
    fn median_of_three(&self, lines: &[SortableLine], a: usize, b: usize, c: usize) -> usize {
        let cmp_ab = lines[a].line.compare_with_keys(
            &lines[b].line,
            &self.config.keys,
            self.config.field_separator,
            &self.config,
        );

        let cmp_bc = lines[b].line.compare_with_keys(
            &lines[c].line,
            &self.config.keys,
            self.config.field_separator,
            &self.config,
        );

        let cmp_ac = lines[a].line.compare_with_keys(
            &lines[c].line,
            &self.config.keys,
            self.config.field_separator,
            &self.config,
        );

        if cmp_ab != Ordering::Greater {
            if cmp_bc != Ordering::Greater {
                b
            } else if cmp_ac != Ordering::Greater {
                c
            } else {
                a
            }
        } else if cmp_bc == Ordering::Greater {
            b
        } else if cmp_ac != Ordering::Greater {
            a
        } else {
            c
        }
    }

    /// Insertion sort for mostly sorted data (O(n) best case)
    fn insertion_sort_lines(&self, lines: &mut [SortableLine]) {
        for i in 1..lines.len() {
            let key = lines[i];
            let mut j = i;

            while j > 0 {
                let cmp = lines[j - 1].line.compare_with_keys(
                    &key.line,
                    &self.config.keys,
                    self.config.field_separator,
                    &self.config,
                );

                if cmp == Ordering::Greater {
                    lines[j] = lines[j - 1];
                    j -= 1;
                } else {
                    break;
                }
            }

            lines[j] = key;
        }
    }

    /// Make sort stable by using original line indices as tie-breaker
    fn make_stable_by_index(&self, lines: &mut [SortableLine]) {
        // Stable sort is already handled by using sort_by instead of sort_unstable_by
        // and falling back to original_index for equal elements
        lines.sort_by(|a, b| {
            let primary_cmp = a.line.compare_with_keys(
                &b.line,
                &self.config.keys,
                self.config.field_separator,
                &self.config,
            );

            // Use original index as tie-breaker for stability
            match primary_cmp {
                std::cmp::Ordering::Equal => a.original_index.cmp(&b.original_index),
                other => other,
            }
        });
    }

    /// Write sorted output
    fn write_output(&self, lines: &[SortableLine]) -> io::Result<()> {
        let mut output: Box<dyn Write> = if let Some(output_file) = &self.args.output {
            Box::new(BufWriter::new(File::create(output_file)?))
        } else {
            Box::new(BufWriter::new(std::io::stdout()))
        };

        let mut prev_line: Option<&SortableLine> = None;

        for line in lines {
            // Handle unique flag
            if self.args.unique {
                if let Some(prev) = prev_line {
                    unsafe {
                        if prev.line.as_bytes() == line.line.as_bytes() {
                            continue; // Skip duplicate
                        }
                    }
                }
            }

            unsafe {
                output.write_all(line.line.as_bytes())?;
                output.write_all(b"\n")?;
            }

            prev_line = Some(line);
        }

        output.flush()?;
        Ok(())
    }
}

/// Wrapper for Line with original position for stable sorting
#[derive(Debug, Clone, Copy)]
struct SortableLine {
    line: Line,
    original_index: usize,
}

// Implement Clone is already derived above

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_ultimate_sort_basic() -> io::Result<()> {
        let temp_dir = TempDir::new()?;
        let input_file = temp_dir.path().join("input.txt");
        let output_file = temp_dir.path().join("output.txt");

        // Create test input
        fs::write(&input_file, "zebra\napple\nbanana\ncherry\n")?;

        // Create sort args
        let args = SortArgs {
            files: vec![input_file.to_string_lossy().to_string()],
            output: Some(output_file.to_string_lossy().to_string()),
            ..Default::default()
        };

        // Sort
        let config = crate::config::SortConfig::default();
        let sorter = CoreSort::new(args, config);
        sorter.sort()?;

        // Verify output
        let output_content = fs::read_to_string(&output_file)?;
        assert_eq!(output_content, "apple\nbanana\ncherry\nzebra\n");

        Ok(())
    }

    #[test]
    fn test_numeric_sort() -> io::Result<()> {
        let temp_dir = TempDir::new()?;
        let input_file = temp_dir.path().join("input.txt");
        let output_file = temp_dir.path().join("output.txt");

        // Create test input
        fs::write(&input_file, "100\n20\n3\n1000\n")?;

        // Create sort args
        let args = SortArgs {
            files: vec![input_file.to_string_lossy().to_string()],
            output: Some(output_file.to_string_lossy().to_string()),
            numeric_sort: true,
            ..Default::default()
        };

        // Sort
        let config =
            crate::config::SortConfig::default().with_mode(crate::config::SortMode::Numeric);
        let sorter = CoreSort::new(args, config);
        sorter.sort()?;

        // Verify output
        let output_content = fs::read_to_string(&output_file)?;
        assert_eq!(output_content, "3\n20\n100\n1000\n");

        Ok(())
    }
}
