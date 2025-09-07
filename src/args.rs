/// Command line arguments for the ultimate sort implementation
#[derive(Debug, Clone)]
pub struct SortArgs {
    pub files: Vec<String>,
    pub output: Option<String>,
    pub reverse: bool,
    pub numeric_sort: bool,
    pub general_numeric_sort: bool, // Added for -g/--general-numeric-sort
    pub random_sort: bool,          // Added for --random-sort support
    pub ignore_case: bool,
    pub unique: bool,
    pub stable: bool,
    pub field_separator: Option<char>,
    pub zero_terminated: bool,
    pub check: bool,
    pub merge: bool,
}

impl Default for SortArgs {
    fn default() -> Self {
        Self {
            files: Vec::new(),
            output: None,
            reverse: false,
            numeric_sort: false,
            general_numeric_sort: false,
            random_sort: false,
            ignore_case: false,
            unique: false,
            stable: false,
            field_separator: None,
            zero_terminated: false,
            check: false,
            merge: false,
        }
    }
}
