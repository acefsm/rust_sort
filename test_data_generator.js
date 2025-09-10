#!/usr/bin/env node
"use strict";

const path = require("path");

const charClasses = {
  ascii: {
    letters: "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz",
    numbers: "0123456789",
    otherPrintables: " !\"#$%&'()*+,-./:;<=>?@[\]^_`{|}~"
  },
  russian: {
    letters: "абвгдеёжзийклмнопрстуфхцчшщъыьэюяАБВГДЕЁЖЗИЙКЛМНОПРСТУФХЦЧШЩЪЫЬЭЮЯ"
  }
};

let charsWritten = 0;
let dictionary = "";
let maxCharsToWrite = 0;
let maxStringLength = 0;
let minStringLength = 0;
let stringCount = 0;

function getRandomIntInclusive(min, max) {
  min = Math.ceil(min);
  max = Math.floor(max);
  return Math.floor(Math.random() * (max - min + 1) + min);
}


if (process.argv.length < 6) {
  process.stderr.write(`Usage: node ${path.basename(process.argv[1])} CHAR_CLASSES MIN_STR_LEN MAX_STR_LEN MAX_CHARS_TO_WRITE

CHAR_CLASSES is a string that defines which character classes to use.
It can contain:
- "e" for ASCII English letters;
- "n" for ASCII numbers;
- "+" for other printable ASCII characters;
- "r" for Russian letters.
You can combine them, e.g. "er+".

Example:
  node ${path.basename(process.argv[1])} rn 5 50 10000
    Write no more than 10000 Russian letters and ASCII numbers,
    each string should be 5 to 50 characters long.
`);
  process.exit(1);
}

dictionary += process.argv[2].includes("e") ? charClasses.ascii.letters : "";
dictionary += process.argv[2].includes("n") ? charClasses.ascii.numbers : "";
dictionary += process.argv[2].includes("+") ? charClasses.ascii.otherPrintables : "";
dictionary += process.argv[2].includes("r") ? charClasses.russian.letters : "";
minStringLength = parseInt(process.argv[3], 10);
maxStringLength = parseInt(process.argv[4]), 10;
maxCharsToWrite = parseInt(process.argv[5]), 10;

if (dictionary.length === 0 ||
    isNaN(minStringLength) ||
    isNaN(maxStringLength) ||
    isNaN(maxCharsToWrite) ||
    minStringLength <= 0 ||
    maxStringLength <= 0 ||
    maxCharsToWrite <= 0 ||
    minStringLength > maxStringLength ||
    maxStringLength >= maxCharsToWrite) {
  process.stderr.write(`Incorrect arguments. Run "node ${path.basename(process.argv[1])}" for help.\n`);
  process.exit(2);
}

while (true) {
  let arr = [...(new Array(getRandomIntInclusive(minStringLength, maxStringLength)))]
      .map(item => dictionary[getRandomIntInclusive(0, dictionary.length - 1)]);

  stringCount += 1;
  charsWritten += arr.length + 1; // + 1 is for "\n"

  if (charsWritten > maxCharsToWrite) {
    break;
  }

  if (stringCount % 10000 === 0) {
    process.stderr.write(`\r${Math.floor(charsWritten / (maxCharsToWrite / 100))}%`);
  }

  process.stdout.write(`${arr.join("")}\n`);
}

process.stderr.write("\r100%\n");
