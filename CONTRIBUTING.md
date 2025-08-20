# Contributing to rust-sort

Thank you for your interest in contributing to rust-sort! This document provides guidelines for contributing to make the process smooth and effective for everyone involved.

## 🎯 Code of Conduct

We are committed to providing a welcoming and inclusive experience for all. Please be respectful and professional in all interactions.

## 🚀 Quick Start

1. **Fork** the repository on GitHub
2. **Clone** your fork locally:
   ```bash
   git clone https://github.com/your-username/rust-sort.git
   cd rust-sort
   ```
3. **Create** a new branch for your feature:
   ```bash
   git checkout -b feature/your-feature-name
   ```
4. **Install** development dependencies:
   ```bash
   rustup update
   cargo install cargo-clippy cargo-fmt
   ```

## 🛠️ Development Workflow

### Setting Up Your Environment

```bash
# Ensure you have the latest stable Rust
rustup update stable

# Install development tools
rustup component add clippy rustfmt

# Run tests to verify setup
cargo test
```

### Before Making Changes

1. **Check existing issues** - see if your idea is already being discussed
2. **Create an issue** for major changes to discuss the approach
3. **Read the code** - understand the existing architecture and patterns

### Making Changes

1. **Write tests first** - we practice test-driven development
2. **Follow the coding style** - use `cargo fmt` and `cargo clippy`
3. **Add documentation** - update docs for any public API changes
4. **Test performance** - run benchmarks for performance-sensitive changes

### Code Quality Standards

#### Rust Code Guidelines

```rust
// ✅ Good: Clear, documented function
/// Sorts the input using adaptive algorithm selection
/// 
/// # Arguments
/// * `data` - The data to sort
/// * `config` - Sorting configuration
/// 
/// # Returns
/// * `Result<(), SortError>` - Success or error details
pub fn adaptive_sort(data: &mut [String], config: &SortConfig) -> SortResult<()> {
    // Implementation
}

// ❌ Bad: Undocumented, unclear function
pub fn sort_stuff(x: &mut [String], y: &SortConfig) -> Result<(), Box<dyn Error>> {
    // Implementation
}
```

#### Performance Considerations

- **Benchmark critical paths** - use `cargo bench` for performance-sensitive code
- **Profile memory usage** - avoid unnecessary allocations
- **Consider SIMD opportunities** - vectorize when possible
- **Minimize system calls** - batch I/O operations

#### Error Handling

```rust
// ✅ Good: Specific error types
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SortError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Invalid configuration: {message}")]
    InvalidConfig { message: String },
}

// ❌ Bad: Generic error handling
fn some_function() -> Result<(), Box<dyn Error>> {
    // Don't use generic error types
}
```

### Testing

#### Unit Tests
```bash
# Run all tests
cargo test

# Run specific test module
cargo test core_sort

# Run tests with output
cargo test -- --nocapture
```

#### Integration Tests
```bash
# Run benchmarks to verify correctness and performance
./benchmark.sh

# Run with large datasets (requires disk space)
./benchmark.sh --large
```

#### Performance Tests
```bash
# Run micro-benchmarks
cargo bench

# Profile performance
cargo build --release
perf record target/release/sort large_file.txt
perf report
```

### Documentation

- **Update README.md** for user-facing changes
- **Add doc comments** for all public APIs
- **Include examples** in documentation
- **Update CHANGELOG.md** for notable changes

## 📋 Types of Contributions

### 🐛 Bug Reports

When reporting bugs, please include:

```markdown
**Describe the bug**
A clear description of what the bug is.

**To Reproduce**
Steps to reproduce the behavior:
1. Command used: `sort -n file.txt`
2. Input data: `[attach or describe]`
3. Expected output: `[what you expected]`
4. Actual output: `[what actually happened]`

**Environment**
- OS: [e.g., Ubuntu 20.04, macOS 12.0]
- Rust version: `rustc --version`
- rust-sort version: `sort --version`

**Additional context**
Any other information about the problem.
```

### ✨ Feature Requests

For new features:

1. **Search existing issues** first
2. **Describe the use case** - why is this feature needed?
3. **Propose the interface** - how should it work?
4. **Consider compatibility** - how does it fit with GNU sort?

### 🏗️ Code Contributions

#### Small Changes
- Bug fixes
- Documentation improvements
- Minor performance optimizations

These can be submitted directly as pull requests.

#### Major Changes
- New sorting algorithms
- Architectural changes
- Breaking API changes

Please create an issue first to discuss the approach.

### 🎯 Good First Issues

Look for issues labeled `good first issue`:

- Documentation improvements
- Adding test cases
- Small bug fixes
- Code cleanup tasks

## 📝 Pull Request Process

### Before Submitting

1. **Run the full test suite**:
   ```bash
   cargo test
   cargo clippy -- -D warnings
   cargo fmt --check
   ./benchmark.sh
   ```

2. **Update documentation** if needed

3. **Add changelog entry** for user-facing changes

### Pull Request Template

```markdown
## Description
Brief description of changes made.

## Type of Change
- [ ] Bug fix (non-breaking change which fixes an issue)
- [ ] New feature (non-breaking change which adds functionality)
- [ ] Breaking change (fix or feature that would cause existing functionality to not work as expected)
- [ ] Documentation update

## Testing
- [ ] Unit tests pass: `cargo test`
- [ ] Linting passes: `cargo clippy`
- [ ] Formatting is correct: `cargo fmt`
- [ ] Benchmarks pass: `./benchmark.sh`
- [ ] Added tests for new functionality

## Performance Impact
- [ ] No performance impact expected
- [ ] Performance improvement (include benchmark results)
- [ ] Potential performance regression (justified by other benefits)

## Checklist
- [ ] My code follows the project's style guidelines
- [ ] I have performed a self-review of my code
- [ ] I have commented my code, particularly in hard-to-understand areas
- [ ] I have made corresponding changes to the documentation
- [ ] My changes generate no new warnings
- [ ] I have added tests that prove my fix is effective or that my feature works
- [ ] New and existing unit tests pass locally with my changes
```

### Review Process

1. **Automated checks** run on all PRs (CI, tests, formatting)
2. **Maintainer review** - we aim to review within 2-3 days
3. **Address feedback** - respond to review comments
4. **Final approval** - maintainer will merge when ready

## 🔧 Specialized Contribution Areas

### Performance Optimization

When contributing performance improvements:

1. **Benchmark before and after**:
   ```bash
   # Before changes
   ./benchmark.sh > before.txt
   
   # After changes
   ./benchmark.sh > after.txt
   
   # Compare results
   diff before.txt after.txt
   ```

2. **Profile bottlenecks**:
   ```bash
   cargo build --release
   perf record -g target/release/sort large_file.txt
   perf report
   ```

3. **Consider memory usage**:
   ```bash
   valgrind --tool=massif target/release/sort large_file.txt
   ```

### Algorithm Implementation

For new sorting algorithms:

1. **Research existing literature** - cite papers or references
2. **Implement with clear documentation** - explain the algorithm
3. **Add comprehensive tests** - edge cases and correctness
4. **Benchmark against existing algorithms** - show when it's beneficial
5. **Consider adaptive integration** - when should this algorithm be used?

### SIMD Optimization

For vectorized code:

1. **Use platform-agnostic code when possible**
2. **Provide fallbacks** for unsupported architectures
3. **Test on multiple platforms** if possible
4. **Document SIMD requirements** clearly

## 🏷️ Commit Guidelines

### Commit Message Format

```
type(scope): description

[optional body]

[optional footer]
```

#### Types
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation only changes
- `style`: Code style changes (formatting, etc.)
- `refactor`: Code refactoring
- `perf`: Performance improvements
- `test`: Adding or updating tests
- `chore`: Maintenance tasks

#### Examples
```bash
feat(radix): add radix sort implementation for integers

fix(simd): handle unaligned memory access correctly

docs(readme): update performance comparison table

perf(core): optimize string comparison with SIMD
```

## 🎉 Recognition

Contributors are recognized in:

- **README.md** - major contributors listed
- **CHANGELOG.md** - contributions noted in releases
- **GitHub contributors page** - automatic recognition

## 📞 Getting Help

- **GitHub Issues** - for bugs and feature requests
- **GitHub Discussions** - for questions and ideas
- **Discord** - real-time chat (link in README)

## 📚 Resources

### Learning Rust
- [The Rust Programming Language](https://doc.rust-lang.org/book/)
- [Rust by Example](https://doc.rust-lang.org/rust-by-example/)
- [The Rustonomicon](https://doc.rust-lang.org/nomicon/) - for unsafe code

### Performance Optimization
- [The Rust Performance Book](https://nnethercote.github.io/perf-book/)
- [Computer Systems: A Programmer's Perspective](http://csapp.cs.cmu.edu/)

### Sorting Algorithms
- *Introduction to Algorithms* by Cormen, Leiserson, Rivest, and Stein
- [Sorting Algorithm Visualizations](https://www.toptal.com/developers/sorting-algorithms)

---

Thank you for contributing to rust-sort! Your efforts help make this tool better for everyone. 🙏