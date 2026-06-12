import { describe, it, expect } from 'vitest';
import type { NodeGraph, GraphNode } from '../shared/types.js';
import {
  flagToVarName,
  flagToPsName,
  buildParamSpecs,
  cliScriptCmd,
  cliScriptPS,
  cliScriptBash,
} from '../main/pipeline/executor-cli.js';

function makeNode(id: string, type: string, params: Record<string, unknown> = {}): GraphNode {
  return { id, type, position: { x: 0, y: 0 }, data: { label: id, definitionId: id, params } };
}

function makeGraph(...nodes: GraphNode[]): NodeGraph {
  return { nodes, edges: [], viewport: { x: 0, y: 0, zoom: 1 } };
}

const DATE = '2026-01-01';
const WF = 'workflow.imgplex';

describe('flagToVarName', () => {
  it('converts hyphenated flag to uppercase underscore', () => {
    expect(flagToVarName('input-1')).toBe('INPUT_1');
  });
  it('handles multi-segment flags', () => {
    expect(flagToVarName('my-long-flag-name')).toBe('MY_LONG_FLAG_NAME');
  });
  it('handles single word', () => {
    expect(flagToVarName('output')).toBe('OUTPUT');
  });
});

describe('flagToPsName', () => {
  it('converts hyphenated flag to PascalCase', () => {
    expect(flagToPsName('input-1')).toBe('Input1');
  });
  it('joins multiple parts', () => {
    expect(flagToPsName('my-long-flag')).toBe('MyLongFlag');
  });
  it('handles single word', () => {
    expect(flagToPsName('output')).toBe('Output');
  });
});

describe('buildParamSpecs', () => {
  it('returns empty arrays when no nodes have cliName', () => {
    const graph = makeGraph(makeNode('i1', 'inputNode'), makeNode('o1', 'imageOutputNode'));
    const { inputs, outputs } = buildParamSpecs(graph);
    expect(inputs).toHaveLength(0);
    expect(outputs).toHaveLength(0);
  });

  it('collects one input and one image output', () => {
    const graph = makeGraph(
      makeNode('i1', 'inputNode', { cliName: 'input-1' }),
      makeNode('o1', 'imageOutputNode', { cliName: 'output-1', outputPath: 'custom', customPath: './out' })
    );
    const { inputs, outputs } = buildParamSpecs(graph);
    expect(inputs).toHaveLength(1);
    expect(inputs[0].flag).toBe('input-1');
    expect(inputs[0].varName).toBe('INPUT_1');
    expect(inputs[0].psName).toBe('Input1');
    expect(inputs[0].required).toBe(true);
    expect(outputs).toHaveLength(1);
    expect(outputs[0].flag).toBe('output-1');
    expect(outputs[0].defaultValue).toBe('./out');
    expect(outputs[0].required).toBe(false);
  });

  it('handles text and flipbook output nodes', () => {
    const graph = makeGraph(
      makeNode('i1', 'inputNode', { cliName: 'src' }),
      makeNode('t1', 'textOutputNode', { cliName: 'report', outputPath: './data.txt' }),
      makeNode('f1', 'flipbookOutputNode', { cliName: 'atlas', flipbookOutputPath: './atlas.png' })
    );
    const { inputs, outputs } = buildParamSpecs(graph);
    expect(inputs).toHaveLength(1);
    expect(outputs).toHaveLength(2);
    expect(outputs[0].defaultValue).toBe('./data.txt');
    expect(outputs[1].defaultValue).toBe('./atlas.png');
  });

  it('skips nodes without cliName', () => {
    const graph = makeGraph(
      makeNode('i1', 'inputNode', { cliName: 'src' }),
      makeNode('p1', 'default', { cliName: 'process' }) // not an input/output type
    );
    const { inputs, outputs } = buildParamSpecs(graph);
    expect(inputs).toHaveLength(1);
    expect(outputs).toHaveLength(0);
  });
});

describe('cliScriptCmd', () => {
  it('includes @echo off header', () => {
    const script = cliScriptCmd(WF, DATE, makeGraph());
    expect(script).toMatch(/^@echo off/);
  });

  it('uses CRLF line endings', () => {
    const script = cliScriptCmd(WF, DATE, makeGraph());
    expect(script).toMatch(/\r\n/);
    expect(script).not.toMatch(/[^\r]\n/);
  });

  it('includes imgplex-cli run invocation', () => {
    const script = cliScriptCmd(WF, DATE, makeGraph());
    expect(script).toContain(`imgplex-cli run`);
    expect(script).toContain(WF);
  });

  it('includes flags for named nodes', () => {
    const graph = makeGraph(
      makeNode('i1', 'inputNode', { cliName: 'src' }),
      makeNode('o1', 'imageOutputNode', { cliName: 'dest' })
    );
    const script = cliScriptCmd(WF, DATE, graph);
    expect(script).toContain('--src');
    expect(script).toContain('--dest');
    expect(script).toContain('SRC');
    expect(script).toContain('DEST');
  });
});

describe('cliScriptPS', () => {
  it('includes # imgplex header', () => {
    const script = cliScriptPS(WF, DATE, makeGraph());
    expect(script).toMatch(/^# imgplex/);
  });

  it('uses LF line endings only', () => {
    const script = cliScriptPS(WF, DATE, makeGraph());
    expect(script).not.toContain('\r\n');
    expect(script).toContain('\n');
  });

  it('includes imgplex-cli run invocation with workflow file', () => {
    const script = cliScriptPS(WF, DATE, makeGraph());
    expect(script).toContain('imgplex-cli run');
    expect(script).toContain(WF);
  });

  it('generates param block for named nodes', () => {
    const graph = makeGraph(makeNode('i1', 'inputNode', { cliName: 'src' }));
    const script = cliScriptPS(WF, DATE, graph);
    expect(script).toContain('param (');
    expect(script).toContain('[string]$Src');
    expect(script).toContain('--src $Src');
  });
});

describe('cliScriptBash', () => {
  it('starts with #!/usr/bin/env bash shebang', () => {
    const script = cliScriptBash(WF, DATE, makeGraph());
    expect(script).toMatch(/^#!\/usr\/bin\/env bash/);
  });

  it('uses LF line endings only', () => {
    const script = cliScriptBash(WF, DATE, makeGraph());
    expect(script).not.toContain('\r\n');
    expect(script).toContain('\n');
  });

  it('includes imgplex-cli run invocation', () => {
    const script = cliScriptBash(WF, DATE, makeGraph());
    expect(script).toContain('imgplex-cli run');
    expect(script).toContain(WF);
  });

  it('includes SCRIPT_DIR definition', () => {
    const script = cliScriptBash(WF, DATE, makeGraph());
    expect(script).toContain('SCRIPT_DIR=');
  });

  it('generates variable assignments for named nodes', () => {
    const graph = makeGraph(makeNode('i1', 'inputNode', { cliName: 'src' }));
    const script = cliScriptBash(WF, DATE, graph);
    expect(script).toContain('SRC=');
    expect(script).toContain('--src');
  });
});

describe('default-value escaping (no script injection)', () => {
  it('PowerShell: a single quote in the path is doubled', () => {
    const graph = makeGraph(
      makeNode('o1', 'imageOutputNode', { cliName: 'dest', outputPath: 'custom', customPath: "C:\\it's\\out" })
    );
    const script = cliScriptPS(WF, DATE, graph);
    expect(script).toContain("= 'C:\\it''s\\out'");
    // No unbalanced single quote that would break the literal.
    expect(script).not.toContain("= 'C:\\it's\\out'");
  });

  it('CMD: a percent sign is doubled to avoid variable expansion', () => {
    const graph = makeGraph(
      makeNode('o1', 'imageOutputNode', { cliName: 'dest', outputPath: 'custom', customPath: 'C:\\100%done' })
    );
    const script = cliScriptCmd(WF, DATE, graph);
    expect(script).toContain('C:\\100%%done');
  });

  it('Bash: $, backtick and quotes are neutralised in the default', () => {
    const graph = makeGraph(
      makeNode('o1', 'imageOutputNode', { cliName: 'dest', outputPath: 'custom', customPath: './out$(whoami)`id`"x"' })
    );
    const script = cliScriptBash(WF, DATE, graph);
    expect(script).toContain('\\$(whoami)');
    expect(script).toContain('\\`id\\`');
    expect(script).toContain('\\"x\\"');
    // Raw command substitution must not survive unescaped.
    expect(script).not.toContain(':-./out$(whoami)');
  });
});
