import { describe, it, expect, vi } from 'vitest';
import { render, fireEvent } from '@testing-library/svelte';
import RunWorkflowDialog from '../renderer/components/RunWorkflowDialog.svelte';
import type { OutputNodeStatus } from '../renderer/components/runWorkflowTypes.js';

const statuses: OutputNodeStatus[] = [
  { nodeId: 'o1', label: 'Image Output', type: 'imageOutputNode', valid: true, reasons: [] },
  {
    nodeId: 'o2',
    label: 'Text Output',
    type: 'textOutputNode',
    valid: false,
    reasons: ['No input wired', 'No output path'],
  },
];

describe('RunWorkflowDialog', () => {
  it('summarises ready vs total and lists invalid reasons', () => {
    const { getByText } = render(RunWorkflowDialog, { statuses, onRun: vi.fn(), onCancel: vi.fn() });
    expect(getByText('1 of 2 output nodes ready to run.', { exact: false })).toBeInTheDocument();
    expect(getByText('No input wired · No output path')).toBeInTheDocument();
  });

  it('runs only valid nodes when the run button is clicked', async () => {
    const onRun = vi.fn();
    const { getByText } = render(RunWorkflowDialog, { statuses, onRun, onCancel: vi.fn() });
    await fireEvent.click(getByText('Run 1 node'));
    expect(onRun).toHaveBeenCalledOnce();
    expect(onRun).toHaveBeenCalledWith(statuses);
  });

  it('disables the run button when nothing is valid', () => {
    const allInvalid: OutputNodeStatus[] = [
      { nodeId: 'o1', label: 'Image Output', type: 'imageOutputNode', valid: false, reasons: ['x'] },
    ];
    const { getByText } = render(RunWorkflowDialog, { statuses: allInvalid, onRun: vi.fn(), onCancel: vi.fn() });
    expect(getByText('Run 0 nodes').closest('button')).toBeDisabled();
  });

  it('cancels via the Cancel button', async () => {
    const onCancel = vi.fn();
    const { getByText } = render(RunWorkflowDialog, { statuses, onRun: vi.fn(), onCancel });
    await fireEvent.click(getByText('Cancel'));
    expect(onCancel).toHaveBeenCalledOnce();
  });
});
