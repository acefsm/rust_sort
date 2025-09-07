/// Command line arguments for the ultimate sort implementation
#[derive(Debug, Clone, Default)]
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
