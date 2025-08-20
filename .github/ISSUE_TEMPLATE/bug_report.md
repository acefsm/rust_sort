---
name: Bug report
about: Create a report to help us improve rust-sort
title: '[BUG] '
labels: ['bug', 'triage']
assignees: ''
---

## 🐛 Bug Description

A clear and concise description of what the bug is.

## 🔄 To Reproduce

**Command used:**
```bash
sort [flags] [input_file]
```

**Input data:**
<!-- Please provide the input data or a minimal example that reproduces the issue -->
```
[paste input data here or attach file]
```

**Steps to reproduce:**
1. Run the command above
2. Observe the output
3. Compare with expected behavior

## ✅ Expected Behavior

A clear description of what you expected to happen.

**Expected output:**
```
[paste expected output here]
```

## ❌ Actual Behavior

What actually happened instead.

**Actual output:**
```
[paste actual output here]
```

**Error messages (if any):**
```
[paste error messages here]
```

## 🖥️ Environment

**Operating System:** [e.g., Ubuntu 22.04, macOS 13.0, Windows 11]

**rust-sort version:** 
```bash
sort --version
```

**Rust version:**
```bash
rustc --version
```

**Hardware:**
- CPU: [e.g., Intel i7-12700K, Apple M2]
- RAM: [e.g., 16GB]
- Architecture: [e.g., x86_64, aarch64]

## 📊 GNU sort Comparison

**Does GNU sort work correctly with the same input?**
- [ ] Yes, GNU sort produces the expected output
- [ ] No, GNU sort has the same issue
- [ ] Not tested

**GNU sort version:** 
```bash
sort --version | head -1
```

**GNU sort output:**
```
[paste GNU sort output here if different]
```

## 📁 Additional Context

**File size:** [e.g., 1000 lines, 50MB]

**Data characteristics:**
- [ ] Numeric data
- [ ] Text data
- [ ] Mixed data
- [ ] Unicode/non-ASCII characters
- [ ] Very long lines (>1000 chars)
- [ ] Many duplicate entries
- [ ] Already partially sorted

**Performance impact:**
- [ ] Correctness issue (wrong output)
- [ ] Performance issue (slower than expected)
- [ ] Memory issue (high memory usage)
- [ ] Crash or panic

**Workarounds found:**
[Describe any workarounds you've discovered]

## 📎 Attachments

<!-- If the input file is large or contains sensitive data, please:
1. Try to create a minimal reproduction case
2. Attach files if they're small (<1MB)
3. Provide a script to generate test data if possible
-->

- [ ] Sample input file attached
- [ ] Sample output file attached
- [ ] Script to reproduce attached

## 🔍 Debugging Information

**Have you tried:**
- [ ] Running with `RUST_BACKTRACE=1`
- [ ] Testing with a smaller input file
- [ ] Checking if the issue is reproducible
- [ ] Running the benchmark suite (`./benchmark.sh`)

**Additional debugging output:**
```
[paste any additional debugging information here]
```

---

**Checklist before submitting:**
- [ ] I've searched existing issues to make sure this isn't a duplicate
- [ ] I've provided a minimal, reproducible example
- [ ] I've tested with the latest version of rust-sort
- [ ] I've compared the behavior with GNU sort
- [ ] I've included all relevant environment information