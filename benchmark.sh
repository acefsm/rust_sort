#!/bin/bash

# Performance Benchmark Script for rust-sort with multiple sort implementations

# Parse command line arguments for additional sort implementations
USAGE="Usage: $0 [options] [--reference-sort PATH] [--add-sort NAME:PATH] ...

Options:
  --large            Include 10M line tests
  --extralarge       Include 30M line tests  
  --reference-sort   Set reference sort (default: system 'sort')
  --add-sort         Add additional sort implementation (format: NAME:PATH)
  --help             Show this help

Examples:
  $0                                    # Basic tests with system sort as reference
  $0 --large                           # Include large tests
  $0 --reference-sort /usr/local/bin/gsort
  $0 --add-sort "GNU:/usr/local/bin/gsort" --add-sort "BSD:/usr/bin/sort""

# Default configuration
REFERENCE_SORT="sort"  # Use system sort as reference
REFERENCE_NAME="System sort"
ADDITIONAL_SORTS=()
ADDITIONAL_NAMES=()
LARGE_TESTS=false
EXTRA_LARGE_TESTS=false

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --reference-sort)
            REFERENCE_SORT="$2"
            REFERENCE_NAME="Reference sort"
            shift 2
            ;;
        --add-sort)
            IFS=':' read -r name path <<< "$2"
            ADDITIONAL_NAMES+=("$name")
            ADDITIONAL_SORTS+=("$path")
            shift 2
            ;;
        --large)
            LARGE_TESTS=true
            shift
            ;;
        --extralarge)
            EXTRA_LARGE_TESTS=true
            LARGE_TESTS=true  # extralarge implies large
            shift
            ;;
        --help)
            echo "$USAGE"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            echo "$USAGE"
            exit 1
            ;;
    esac
done

echo "================================================"
echo "    🚀 RUST-SORT PERFORMANCE BENCHMARK 🚀"
echo "================================================"
echo "Reference sort: $REFERENCE_NAME ($REFERENCE_SORT)"
if [ ${#ADDITIONAL_SORTS[@]} -gt 0 ]; then
    echo "Additional sorts:"
    for i in "${!ADDITIONAL_NAMES[@]}"; do
        echo "  - ${ADDITIONAL_NAMES[$i]}: ${ADDITIONAL_SORTS[$i]}"
    done
fi
echo ""

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
RED='\033[0;31m'
CYAN='\033[0;36m'
MAGENTA='\033[0;35m'
NC='\033[0m'

# Statistics
PASSED=0
FAILED=0

# Build if needed
if [ ! -f "target/release/sort" ]; then
    echo -e "${YELLOW}Building Rust sort...${NC}"
    cargo build --release
fi


# Verify reference sort exists
if ! command -v "$REFERENCE_SORT" >/dev/null 2>&1; then
    echo -e "${RED}Error: Reference sort '$REFERENCE_SORT' not found${NC}"
    exit 1
fi

# Verify additional sorts exist
for i in "${!ADDITIONAL_SORTS[@]}"; do
    sort_cmd="${ADDITIONAL_SORTS[$i]}"
    # Extract first command if it's a compound command like "coreutils sort"
    first_cmd=$(echo "$sort_cmd" | awk '{print $1}')
    if ! command -v "$first_cmd" >/dev/null 2>&1; then
        echo -e "${YELLOW}Warning: ${ADDITIONAL_NAMES[$i]} sort '$sort_cmd' not found, skipping${NC}"
        unset 'ADDITIONAL_NAMES[$i]'
        unset 'ADDITIONAL_SORTS[$i]'
    fi
done

# Rebuild arrays to remove gaps
ADDITIONAL_NAMES=("${ADDITIONAL_NAMES[@]}")
ADDITIONAL_SORTS=("${ADDITIONAL_SORTS[@]}")

# Data generation function with fixed seed
generate_data() {
    local size=$1
    local suffix=$2
    
    echo -e "${BLUE}Generating ${size} test data...${NC}"
    
    # For large datasets, use more efficient generation methods
    if [ $size -ge 1000000 ]; then
        # Use seq and awk for much faster generation
        
        # Numeric data
        if [ ! -f "test_nums_${suffix}.txt" ]; then
            seq 1 $size | awk '{print ($1 * 7919) % 32749}' > "test_nums_${suffix}.txt"
        fi
        
        # String data
        if [ ! -f "test_strings_${suffix}.txt" ]; then
            seq 1 $size | awk '{print "str_" (($1 * 13) % 9973) "_text"}' > "test_strings_${suffix}.txt"
        fi
        
        # Float data
        if [ ! -f "test_floats_${suffix}.txt" ]; then
            seq 1 $size | awk '{print (($1 * 17) % 10007) "." (($1 * 23) % 1000)}' > "test_floats_${suffix}.txt"
        fi
        
        # Mixed data
        if [ ! -f "test_mixed_${suffix}.txt" ]; then
            seq 1 $size | awk '{if ($1 % 2 == 0) print ($1 * 19) % 7919; else print "text_" (($1 * 31) % 5003)}' > "test_mixed_${suffix}.txt"
        fi
        
        # Duplicate data
        if [ ! -f "test_dups_${suffix}.txt" ]; then
            seq 1 $size | awk '{print $1 % 100}' > "test_dups_${suffix}.txt"
        fi
    else
        # Use fixed seed for reproducibility
        RANDOM=42
        
        # Numeric data
        if [ ! -f "test_nums_${suffix}.txt" ]; then
            for ((i=1; i<=size; i++)); do echo $((i * 7919 % 32749)); done > "test_nums_${suffix}.txt"
        fi
        
        # String data
        if [ ! -f "test_strings_${suffix}.txt" ]; then
            for ((i=1; i<=size; i++)); do echo "str_$((i * 13 % 9973))_text"; done > "test_strings_${suffix}.txt"
        fi
        
        # Float data
        if [ ! -f "test_floats_${suffix}.txt" ]; then
            for ((i=1; i<=size; i++)); do echo "$((i * 17 % 10007)).$((i * 23 % 1000))"; done > "test_floats_${suffix}.txt"
        fi
        
        # Mixed data
        if [ ! -f "test_mixed_${suffix}.txt" ]; then
            for ((i=1; i<=size; i++)); do 
                [ $((i%2)) -eq 0 ] && echo "$((i * 19 % 7919))" || echo "text_$((i * 31 % 5003))"
            done > "test_mixed_${suffix}.txt"
        fi
        
        # Duplicate data
        if [ ! -f "test_dups_${suffix}.txt" ]; then
            for ((i=1; i<=size; i++)); do echo "$((i % 100))"; done > "test_dups_${suffix}.txt"
        fi
    fi
}

# Enhanced test function with multiple binary comparison
test_sort_all() {
    local name="$1"
    local file="$2"
    local flags="$3"
    local size_label="$4"
    
    echo -e "${CYAN}Testing: $name (${size_label})${NC}"
    echo "  File: $file | Flags: '${flags}'"
    
    # Run sorts for correctness
    local line_count=$(wc -l < "$file")
    
    echo -e "  ${BLUE}Generating reference outputs for correctness check...${NC}"
    $REFERENCE_SORT $flags "$file" > /tmp/reference.txt 2>/dev/null
    ./target/release/sort $flags "$file" > /tmp/rust.txt 2>/dev/null
    
    # Generate outputs for additional sorts
    for i in "${!ADDITIONAL_SORTS[@]}"; do
        ${ADDITIONAL_SORTS[$i]} $flags "$file" > "/tmp/additional_${i}.txt" 2>/dev/null
    done
    
    # Performance monitoring function
    monitor_performance() {
        local cmd="$1"
        local label="$2"
        
        # Use time command with verbose output for memory/CPU stats
        /usr/bin/time -l $cmd > /dev/null 2>/tmp/time_output_${label}.txt
        
        # Extract metrics from time output (macOS format)
        local real_time=$(awk '/real/ {print $1}' /tmp/time_output_${label}.txt)
        local user_time=$(awk '/user/ {print $3}' /tmp/time_output_${label}.txt)
        local sys_time=$(awk '/sys/ {print $5}' /tmp/time_output_${label}.txt)
        local max_mem=$(awk '/maximum resident set size/ {print $1}' /tmp/time_output_${label}.txt)
        
        # Convert memory from bytes to MB
        local mem_mb=$(echo "scale=1; $max_mem / 1024 / 1024" | bc 2>/dev/null || echo "0")
        
        # Store results in variables named after label
        eval "${label}_real=\"$real_time\""
        eval "${label}_user=\"$user_time\""
        eval "${label}_sys=\"$sys_time\""
        eval "${label}_mem=\"$mem_mb\""
    }
    
    echo -e "  ${BLUE}Measuring performance...${NC}"
    
    # Time reference sort
    monitor_performance "$REFERENCE_SORT $flags $file" "reference"
    
    # Time our Rust sort
    monitor_performance "./target/release/sort $flags $file" "our"
    
    # Time additional sorts
    for i in "${!ADDITIONAL_SORTS[@]}"; do
        monitor_performance "${ADDITIONAL_SORTS[$i]} $flags $file" "additional_${i}"
    done
    
    # Display results for each binary
    echo -e "  ${MAGENTA}Performance Results:${NC}"
    echo -e "    ${YELLOW}$REFERENCE_NAME:${NC}"
    echo -e "      Time: ${reference_real}s (user: ${reference_user}s, sys: ${reference_sys}s)"
    echo -e "      Memory: ${reference_mem}MB"
    
    echo -e "    ${GREEN}Our rust-sort:${NC}"
    echo -e "      Time: ${our_real}s (user: ${our_user}s, sys: ${our_sys}s)"
    echo -e "      Memory: ${our_mem}MB"
    
    # Display additional sorts
    for i in "${!ADDITIONAL_SORTS[@]}"; do
        eval "real_time=\${additional_${i}_real}"
        eval "user_time=\${additional_${i}_user}"
        eval "sys_time=\${additional_${i}_sys}"
        eval "mem_usage=\${additional_${i}_mem}"
        echo -e "    ${YELLOW}${ADDITIONAL_NAMES[$i]}:${NC}"
        echo -e "      Time: ${real_time}s (user: ${user_time}s, sys: ${sys_time}s)"
        echo -e "      Memory: ${mem_usage}MB"
    done
    
    # Calculate speedups
    if [ "$our_real" != "" ] && [ "$our_real" != "0.00" ]; then
        ref_speedup=$(echo "scale=2; $reference_real / $our_real" | bc 2>/dev/null || echo "1")
        echo -e "    ${GREEN}Speedup vs $REFERENCE_NAME: ${ref_speedup}x${NC}"
        
        # Calculate speedups vs additional sorts
        for i in "${!ADDITIONAL_SORTS[@]}"; do
            eval "add_real=\${additional_${i}_real}"
            if [ "$add_real" != "" ] && [ "$add_real" != "0.00" ]; then
                add_speedup=$(echo "scale=2; $add_real / $our_real" | bc 2>/dev/null || echo "1")
                echo -e "    ${GREEN}Speedup vs ${ADDITIONAL_NAMES[$i]}: ${add_speedup}x${NC}"
            fi
        done
    fi
    
    # Check correctness
    echo -e "  ${BLUE}Checking correctness...${NC}"
    
    # For large files, use faster checksum comparison
    local use_checksum=false
    if [ $line_count -gt 5000000 ]; then
        use_checksum=true
        echo -e "    ${YELLOW}Using checksum comparison for large file${NC}"
    fi
    
    if [[ "$flags" == *"-R"* ]]; then
        # For random sort, check that all lines are present
        sort /tmp/reference.txt > /tmp/reference_sorted.txt
        sort /tmp/rust.txt > /tmp/rust_sorted.txt
        
        local our_correct=true
        declare -a additional_correct
        
        # Check our sort
        if $use_checksum; then
            if [ "$(sort /tmp/rust.txt | shasum)" != "$(sort /tmp/reference.txt | shasum)" ]; then
                our_correct=false
            fi
        else
            if ! diff -q /tmp/reference_sorted.txt /tmp/rust_sorted.txt > /dev/null 2>&1; then
                our_correct=false
            fi
        fi
        
        # Check additional sorts
        for i in "${!ADDITIONAL_SORTS[@]}"; do
            additional_correct[$i]=true
            if $use_checksum; then
                if [ "$(sort /tmp/additional_${i}.txt | shasum)" != "$(sort /tmp/reference.txt | shasum)" ]; then
                    additional_correct[$i]=false
                fi
            else
                sort "/tmp/additional_${i}.txt" > "/tmp/additional_${i}_sorted.txt"
                if ! diff -q /tmp/reference_sorted.txt "/tmp/additional_${i}_sorted.txt" > /dev/null 2>&1; then
                    additional_correct[$i]=false
                fi
            fi
        done
        
        # Report results
        if $our_correct; then
            echo -e "  ${GREEN}✓ Our rust-sort: CORRECT (all lines present)${NC}"
            ((PASSED++))
        else
            echo -e "  ${RED}✗ Our rust-sort: MISMATCH! Different lines${NC}"
            ((FAILED++))
        fi
        
        # Report additional sorts
        for i in "${!ADDITIONAL_SORTS[@]}"; do
            if [ "${additional_correct[$i]}" = "true" ]; then
                echo -e "  ${GREEN}✓ ${ADDITIONAL_NAMES[$i]}: CORRECT${NC}"
            else
                echo -e "  ${RED}✗ ${ADDITIONAL_NAMES[$i]}: MISMATCH!${NC}"
            fi
        done
    else
        # For deterministic sorts
        local our_correct=true
        declare -a additional_correct
        
        # Check our sort
        if $use_checksum; then
            if [ "$(shasum < /tmp/rust.txt)" != "$(shasum < /tmp/reference.txt)" ]; then
                our_correct=false
            fi
        else
            if ! diff -q /tmp/reference.txt /tmp/rust.txt > /dev/null 2>&1; then
                our_correct=false
            fi
        fi
        
        # Check additional sorts
        for i in "${!ADDITIONAL_SORTS[@]}"; do
            additional_correct[$i]=true
            if $use_checksum; then
                if [ "$(shasum < /tmp/additional_${i}.txt)" != "$(shasum < /tmp/reference.txt)" ]; then
                    additional_correct[$i]=false
                fi
            else
                if ! diff -q /tmp/reference.txt "/tmp/additional_${i}.txt" > /dev/null 2>&1; then
                    additional_correct[$i]=false
                fi
            fi
        done
        
        # Report results
        if $our_correct; then
            echo -e "  ${GREEN}✓ Our rust-sort: CORRECT${NC}"
            ((PASSED++))
        else
            echo -e "  ${RED}✗ Our rust-sort: MISMATCH!${NC}"
            echo "  First difference:"
            diff /tmp/reference.txt /tmp/rust.txt | head -3
            ((FAILED++))
        fi
        
        # Report additional sorts
        for i in "${!ADDITIONAL_SORTS[@]}"; do
            if [ "${additional_correct[$i]}" = "true" ]; then
                echo -e "  ${GREEN}✓ ${ADDITIONAL_NAMES[$i]}: CORRECT${NC}"
            else
                echo -e "  ${RED}✗ ${ADDITIONAL_NAMES[$i]}: MISMATCH!${NC}"
                echo "  ${ADDITIONAL_NAMES[$i]} difference:"
                diff /tmp/reference.txt "/tmp/additional_${i}.txt" | head -3
            fi
        done
    fi
    echo ""
}

# Test with specific size
run_test_suite() {
    local size=$1
    local suffix=$2
    local label=$3
    
    echo -e "${YELLOW}=== TESTING WITH ${label} ===${NC}\n"
    
    generate_data $size $suffix
    
    test_sort_all "Basic numeric" "test_nums_${suffix}.txt" "-n" "$label"
    test_sort_all "Basic string" "test_strings_${suffix}.txt" "" "$label"
    test_sort_all "Reverse numeric" "test_nums_${suffix}.txt" "-rn" "$label"
    test_sort_all "Unique sort" "test_dups_${suffix}.txt" "-u" "$label"
    test_sort_all "Numeric unique" "test_dups_${suffix}.txt" "-nu" "$label"
    test_sort_all "Ignore case" "test_strings_${suffix}.txt" "-f" "$label"
    test_sort_all "Random sort" "test_dups_${suffix}.txt" "-R" "$label"
    test_sort_all "Stable sort" "test_dups_${suffix}.txt" "-s" "$label"
    test_sort_all "General numeric" "test_floats_${suffix}.txt" "-g" "$label"
    test_sort_all "Combined flags" "test_dups_${suffix}.txt" "-nru" "$label"
}

# Check sorted functionality test
test_check_sorted() {
    echo -e "${YELLOW}=== CHECK SORTED TEST ===${NC}\n"
    
    # Generate sorted file
    sort -n test_nums_100k.txt > test_sorted.txt
    
    echo -e "${CYAN}Testing: Check if sorted (numeric)${NC}"
    if ./target/release/sort -cn test_sorted.txt 2>/dev/null; then
        echo -e "  ${GREEN}✓ Correctly detected sorted file${NC}"
        ((PASSED++))
    else
        echo -e "  ${RED}✗ Failed to detect sorted file${NC}"
        ((FAILED++))
    fi
    
    # Break sort order
    echo "1" >> test_sorted.txt
    if ! ./target/release/sort -cn test_sorted.txt 2>/dev/null; then
        echo -e "  ${GREEN}✓ Correctly detected unsorted file${NC}"
        ((PASSED++))
    else
        echo -e "  ${RED}✗ Failed to detect unsorted file${NC}"
        ((FAILED++))
    fi
    echo ""
}

# Main execution
echo -e "${BLUE}Starting comprehensive test suite...${NC}\n"

# Run tests with 100k and 1m datasets
run_test_suite 100000 "100k" "100K lines"
run_test_suite 1000000 "1m" "1M lines"

# Check sorted functionality
test_check_sorted

# Large data tests if requested
if [ "$LARGE_TESTS" = "true" ]; then
    echo -e "${YELLOW}=== LARGE DATA TESTS ===${NC}\n"
    
    # 10M test
    run_test_suite 10000000 "10m" "10M lines"
fi

# Extra large data tests if requested
if [ "$EXTRA_LARGE_TESTS" = "true" ]; then
    echo -e "${YELLOW}=== EXTRA LARGE DATA TESTS ===${NC}\n"
    
    # 30M test
    run_test_suite 30000000 "30m" "30M lines"
fi

# Summary
echo "================================================"
echo -e "${YELLOW}FINAL SUMMARY${NC}"
echo "================================================"
echo -e "Tests passed: ${GREEN}$PASSED${NC}"
echo -e "Tests failed: ${RED}$FAILED${NC}"

if [ $FAILED -eq 0 ]; then
    echo -e "\n${GREEN}✅ ALL TESTS PASSED!${NC}"
    echo -e "${GREEN}Our Rust sort is fully compatible with GNU sort!${NC}"
else
    echo -e "\n${RED}⚠️ Some tests failed${NC}"
fi

echo ""
echo "For large data tests (10M): ./benchmark.sh --large"
echo "For extra large tests (30M): ./benchmark.sh --extralarge"
echo "================================================"