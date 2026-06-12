// CLI script export — generates shell scripts based on the workflow graph.
// Each inputNode and output node with a cliName becomes a named flag in the script.
// The companion .imgplex workflow file is saved alongside the script by the export handler.

import type { NodeGraph } from '../../shared/types.js';

type GraphNode = NodeGraph['nodes'][number];

function nodeCliName(node: GraphNode): string {
  return ((node.data.params.cliName as string) ?? '').trim();
}

type ParamSpec = {
  flag: string; // e.g. 'input-1'
  varName: string; // e.g. 'INPUT_1' (bash/cmd) or 'Input1' (PS)
  psName: string; // e.g. 'Input1'
  description: string;
  defaultValue: string;
  required: boolean;
};

// Per-shell escaping for user-entered default values (output paths). Without
// these, a path containing a quote or shell metacharacter produces an invalid or
// unsafe generated script.
function escapeCmd(v: string): string {
  // In `set "VAR=..."`, only % triggers expansion; " is illegal in Windows paths.
  return v.replace(/%/g, '%%');
}
function escapePs(v: string): string {
  // Single-quoted PowerShell string: a literal quote is written as two quotes.
  return v.replace(/'/g, "''");
}
function escapeBashDq(v: string): string {
  // Inside a double-quoted `${n:-default}`, neutralise expansion and command subst.
  return v.replace(/[\\$`"]/g, (c) => '\\' + c);
}

export function flagToVarName(flag: string): string {
  return flag.toUpperCase().replace(/-/g, '_');
}

export function flagToPsName(flag: string): string {
  return flag
    .split('-')
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join('');
}

export function buildParamSpecs(graph: NodeGraph): { inputs: ParamSpec[]; outputs: ParamSpec[] } {
  const INPUT_TYPES = new Set(['inputNode']);
  const OUTPUT_TYPES = new Set(['imageOutputNode', 'textOutputNode', 'flipbookOutputNode']);

  const inputs: ParamSpec[] = [];
  const outputs: ParamSpec[] = [];

  for (const node of graph.nodes) {
    const name = nodeCliName(node);
    if (!name) continue;

    if (INPUT_TYPES.has(node.type)) {
      inputs.push({
        flag: name,
        varName: flagToVarName(name),
        psName: flagToPsName(name),
        description: 'Source image folder',
        defaultValue: '.',
        required: true,
      });
    } else if (OUTPUT_TYPES.has(node.type)) {
      let defaultValue = '';
      if (node.type === 'imageOutputNode') {
        const outputPath = (node.data.params.outputPath as string) ?? 'source';
        defaultValue = outputPath === 'custom' ? ((node.data.params.customPath as string) ?? './output') : './output';
      } else if (node.type === 'textOutputNode') {
        defaultValue = (node.data.params.outputPath as string) ?? './output.txt';
      } else if (node.type === 'flipbookOutputNode') {
        defaultValue = (node.data.params.flipbookOutputPath as string) ?? './flipbook.png';
      }
      const isDir = node.type === 'imageOutputNode';
      outputs.push({
        flag: name,
        varName: flagToVarName(name),
        psName: flagToPsName(name),
        description: isDir ? 'Output folder' : 'Output file path',
        defaultValue: defaultValue || (isDir ? './output' : './output.txt'),
        required: false,
      });
    }
  }

  return { inputs, outputs };
}

export function cliScriptCmd(workflowFileName: string, date: string, graph: NodeGraph): string {
  const { inputs, outputs } = buildParamSpecs(graph);
  const allParams = [...inputs, ...outputs];

  const lines: string[] = ['@echo off', ':: imgplex — Generated Batch Script', `:: Generated: ${date}`, '::'];

  if (allParams.length > 0) {
    lines.push(':: Usage: script.bat [flags]');
    lines.push('::');
    lines.push(':: Flags (positional order):');
    allParams.forEach((p, i) => {
      const req = p.required ? '(required)' : `(default: ${p.defaultValue})`;
      lines.push(`::   %${i + 1}  --${p.flag}   ${p.description} ${req}`);
    });
  } else {
    lines.push(':: Usage: script.bat');
  }

  lines.push('::', ':: Requires imgplex to be installed. imgplex-cli is added to PATH automatically.', '');

  allParams.forEach((p, i) => {
    lines.push(`set "${p.varName}=%~${i + 1}"`);
    lines.push(`if "%${p.varName}%"=="" set "${p.varName}=${escapeCmd(p.defaultValue)}"`);
  });

  if (allParams.length > 0) lines.push('');

  const flagArgs = allParams.map((p) => `--${p.flag} "%${p.varName}%"`).join(' ');
  lines.push(`imgplex-cli run "%~dp0${escapeCmd(workflowFileName)}"${flagArgs ? ' ' + flagArgs : ''}`);

  return lines.join('\r\n') + '\r\n'; // CRLF for Windows
}

export function cliScriptPS(workflowFileName: string, date: string, graph: NodeGraph): string {
  const { inputs, outputs } = buildParamSpecs(graph);
  const allParams = [...inputs, ...outputs];

  const lines: string[] = ['# imgplex — Generated PowerShell Script', `# Generated: ${date}`, '#'];

  if (allParams.length > 0) {
    lines.push('# Usage: .\\script.ps1 [-FlagName "value"] …');
    lines.push('#');
    allParams.forEach((p) => {
      const req = p.required ? 'required' : `default: ${p.defaultValue}`;
      lines.push(`#   -${p.psName.padEnd(20)} ${p.description} (${req})`);
    });
  } else {
    lines.push('# Usage: .\\script.ps1');
  }

  lines.push('#', '# Requires imgplex to be installed. imgplex-cli is added to PATH automatically.', '');

  if (allParams.length > 0) {
    lines.push('param (');
    allParams.forEach((p, i) => {
      const comma = i < allParams.length - 1 ? ',' : '';
      lines.push(`  [string]$${p.psName} = '${escapePs(p.defaultValue)}'${comma}`);
    });
    lines.push(')', '');
  }

  lines.push(`$WorkflowFile = Join-Path $PSScriptRoot '${escapePs(workflowFileName)}'`);

  const flagArgs = allParams.map((p) => `--${p.flag} $${p.psName}`).join(' ');
  lines.push(`imgplex-cli run $WorkflowFile${flagArgs ? ' ' + flagArgs : ''}`);

  return lines.join('\n') + '\n';
}

export function cliScriptBash(workflowFileName: string, date: string, graph: NodeGraph): string {
  const { inputs, outputs } = buildParamSpecs(graph);
  const allParams = [...inputs, ...outputs];

  const lines: string[] = ['#!/usr/bin/env bash', '# imgplex — Generated Shell Script', `# Generated: ${date}`, '#'];

  if (allParams.length > 0) {
    lines.push('# Usage: bash script.sh [positional values]');
    lines.push('#');
    allParams.forEach((p, i) => {
      const req = p.required ? 'required' : `default: ${p.defaultValue}`;
      lines.push(`#   $${i + 1}  --${p.flag}   ${p.description} (${req})`);
    });
  } else {
    lines.push('# Usage: bash script.sh');
  }

  lines.push('#', '# Requires imgplex to be installed. imgplex-cli must be available in PATH.', '');

  lines.push('SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"');

  allParams.forEach((p, i) => {
    lines.push(`${p.varName}="\${${i + 1}:-${escapeBashDq(p.defaultValue)}}"`);
  });

  if (allParams.length > 0) lines.push('');

  const flagArgs = allParams.map((p) => `--${p.flag} "\${${p.varName}}"`).join(' \\\n  ');
  if (flagArgs) {
    lines.push(`imgplex-cli run "\${SCRIPT_DIR}/${escapeBashDq(workflowFileName)}" \\`);
    lines.push(`  ${flagArgs}`);
  } else {
    lines.push(`imgplex-cli run "\${SCRIPT_DIR}/${escapeBashDq(workflowFileName)}"`);
  }

  return lines.join('\n') + '\n';
}
