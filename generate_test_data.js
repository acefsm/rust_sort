#!/usr/bin/env node
"use strict";

/**
 * Test data generator for rust-sort benchmarks
 * Generates random strings of variable length for performance and correctness testing
 */

const minStringLength = 5;
const maxStringLength = 50;

function getRandomIntInclusive(min, max) {
  min = Math.ceil(min);
  max = Math.floor(max);
  return Math.floor(Math.random() * (max - min + 1) + min);
}

function printUsage() {
  console.error("Usage: node generate_test_data.js <number_of_lines> [output_file]");
  console.error("");
  console.error("Examples:");
  console.error("  node generate_test_data.js 1000000 > test_1m.txt");
  console.error("  node generate_test_data.js 5000000 large_test.txt");
  console.error("  node generate_test_data.js 100  # Small test to stdout");
  process.exit(1);
}

// Parse command line arguments
const args = process.argv.slice(2);
if (args.length === 0 || args[0] === '--help' || args[0] === '-h') {
  printUsage();
}

const numLines = parseInt(args[0]);
if (isNaN(numLines) || numLines <= 0) {
  console.error("Error: Number of lines must be a positive integer");
  printUsage();
}

const outputFile = args[1];

// Set up output stream
let output = process.stdout;
if (outputFile) {
  const fs = require('fs');
  output = fs.createWriteStream(outputFile);
}

// Progress reporting for large files
const reportProgress = numLines >= 100000;
let progressReported = 0;

// Generate test data
console.error(`Generating ${numLines.toLocaleString()} random strings...`);
const startTime = Date.now();

for (let i = 0; i < numLines; i += 1) {
  // Generate random string of random length
  const stringLength = getRandomIntInclusive(minStringLength, maxStringLength);
  const arr = new Array(stringLength);
  
  for (let j = 0; j < stringLength; j += 1) {
    // Generate printable ASCII characters (32-126)
    arr[j] = getRandomIntInclusive(32, 126);
  }
  
  // Write line
  output.write(String.fromCharCode(...arr) + "\n");
  
  // Progress reporting
  if (reportProgress && i > 0 && i % 100000 === 0) {
    const progress = ((i / numLines) * 100).toFixed(1);
    console.error(`Progress: ${i.toLocaleString()}/${numLines.toLocaleString()} (${progress}%)`);
    progressReported = i;
  }
}

const endTime = Date.now();
const duration = (endTime - startTime) / 1000;

// Final progress report
if (reportProgress && progressReported < numLines) {
  console.error(`Progress: ${numLines.toLocaleString()}/${numLines.toLocaleString()} (100.0%)`);
}

console.error(`Generated ${numLines.toLocaleString()} lines in ${duration.toFixed(2)}s`);

// Close output file if specified
if (outputFile) {
  output.end();
  // Wait a bit for file to be written, then get stats
  setTimeout(() => {
    try {
      const fs = require('fs');
      const stats = fs.statSync(outputFile);
      const sizeM = (stats.size / (1024 * 1024)).toFixed(1);
      console.error(`Output file: ${outputFile} (${sizeM}MB)`);
    } catch (e) {
      // File might not be flushed yet, that's ok
    }
  }, 100);
}