// CLI script export functions — pure string generation, no I/O.
// Each generated script delegates all processing to imgplex-cli, which is
// installed alongside the imgplex app and added to the user's PATH by the installer.
// The companion .imgplex workflow file is saved alongside the script by the export handler.

export function cliScriptCmd(workflowFileName: string, date: string): string {
  return [
    '@echo off',
    ':: imgplex — Generated Batch Script',
    `:: Generated: ${date}`,
    '::',
    ':: Usage: script.bat [INPUT_DIR] [OUTPUT_DIR]',
    '::',
    '::   INPUT_DIR   Folder of images to process  (default: current directory)',
    '::   OUTPUT_DIR  Destination folder            (default: output)',
    '::',
    ':: Requires imgplex to be installed. imgplex-cli is added to PATH automatically.',
    '',
    'set "INPUT_DIR=%~1"',
    'set "OUTPUT_DIR=%~2"',
    'if "%INPUT_DIR%"==""  set "INPUT_DIR=."',
    'if "%OUTPUT_DIR%"=="" set "OUTPUT_DIR=output"',
    '',
    `imgplex-cli run "%~dp0${workflowFileName}" --input "%INPUT_DIR%" --output "%OUTPUT_DIR%"`,
  ].join('\r\n') + '\r\n'   // CRLF for Windows
}

export function cliScriptPS(workflowFileName: string, date: string): string {
  return [
    '# imgplex — Generated PowerShell Script',
    `# Generated: ${date}`,
    '#',
    '# Usage: .\\script.ps1 [-InputDir "C:\\Photos"] [-OutputDir "C:\\Output"]',
    '#',
    '# Requires imgplex to be installed. imgplex-cli is added to PATH automatically.',
    '',
    'param (',
    "  [string]$InputDir  = '.'",
    "  [string]$OutputDir = '.\\output'",
    ')',
    '',
    `$WorkflowFile = Join-Path $PSScriptRoot '${workflowFileName}'`,
    'imgplex-cli run $WorkflowFile --input $InputDir --output $OutputDir',
  ].join('\n') + '\n'
}

export function cliScriptBash(workflowFileName: string, date: string): string {
  return [
    '#!/usr/bin/env bash',
    '# imgplex — Generated Shell Script',
    `# Generated: ${date}`,
    '#',
    '# Usage: bash script.sh [INPUT_DIR] [OUTPUT_DIR]',
    '#',
    '#   INPUT_DIR   Folder of images to process  (default: current directory)',
    '#   OUTPUT_DIR  Destination folder            (default: ./output)',
    '#',
    '# Requires imgplex to be installed. imgplex-cli must be available in PATH.',
    '',
    'INPUT_DIR="${1:-.}"',
    'OUTPUT_DIR="${2:-./output}"',
    'SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"',
    '',
    `imgplex-cli run "\${SCRIPT_DIR}/${workflowFileName}" --input "\${INPUT_DIR}" --output "\${OUTPUT_DIR}"`,
  ].join('\n') + '\n'
}
