<script lang="ts">
  let {
    value,
    options,
    labels,
    disabled = false,
    onchange,
  }: {
    value: string;
    options: string[];
    labels?: string[]; // optional display labels; falls back to options[i]
    disabled?: boolean;
    onchange: (v: string) => void;
  } = $props();

  let open = $state(false);

  function select(opt: string) {
    onchange(opt);
    open = false;
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') open = false;
    else if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      open = !open;
    } else if (e.key === 'ArrowDown') {
      e.preventDefault();
      const i = options.indexOf(value);
      if (i < options.length - 1) select(options[i + 1]);
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      const i = options.indexOf(value);
      if (i > 0) select(options[i - 1]);
    }
  }

  function displayLabel(opt: string, i: number): string {
    return labels?.[i] ?? opt;
  }

  const currentLabel = $derived(labels?.[options.indexOf(value)] ?? value);
</script>

<div class="dd-wrap" class:open>
  <button class="dd-btn" {disabled} onclick={() => (open = !open)} onkeydown={onKeydown} onblur={() => (open = false)}>
    <span>{currentLabel}</span>
    <svg width="8" height="5" viewBox="0 0 8 5" class:flipped={open}>
      <path
        d="M1 1l3 3 3-3"
        stroke="currentColor"
        stroke-width="1.2"
        fill="none"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
    </svg>
  </button>
  {#if open}
    <div class="dd-list" role="listbox">
      {#each options as opt, i}
        <div
          class="dd-item"
          class:active={opt === value}
          role="option"
          aria-selected={opt === value}
          tabindex="-1"
          onmousedown={(e) => {
            e.preventDefault();
            select(opt);
          }}
        >
          {displayLabel(opt, i)}
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .dd-wrap {
    position: relative;
    width: 100%;
  }

  .dd-btn {
    display: flex;
    align-items: center;
    justify-content: space-between;
    width: 100%;
    background: var(--dropdown-bg);
    border: 2px solid var(--border);
    border-radius: 4px;
    color: var(--text-bright);
    font-family: var(--font-ui);
    font-size: var(--font-size-base);
    height: 30px;
    padding: 0 8px;
    cursor: pointer;
    outline: none;
    text-align: left;
    transition:
      background 0.1s,
      border-color 0.1s;
  }

  .dd-btn:hover:not(:disabled) {
    background: var(--dropdown-hover-bg);
    border-color: var(--accent);
  }
  .dd-btn svg {
    opacity: 0.5;
    transition: transform 0.15s;
    flex-shrink: 0;
  }
  .dd-btn svg.flipped {
    transform: rotate(180deg);
  }
  .dd-btn:focus {
    border-color: var(--accent);
  }
  .dd-btn:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .dd-list {
    position: absolute;
    top: calc(100% + 2px);
    left: 0;
    right: 0;
    background: var(--dropdown-bg);
    border: 2px solid var(--border);
    border-radius: 4px;
    overflow: hidden;
    z-index: 100;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.4);
  }

  .dd-item {
    padding: 5px 10px;
    font-family: var(--font-ui);
    font-size: var(--font-size-base);
    color: var(--text-bright);
    cursor: pointer;
    user-select: none;
    transition:
      background 0.08s,
      color 0.08s;
  }

  .dd-item:hover {
    background: var(--dropdown-active-bg);
    color: var(--text-bright);
  }
  .dd-item.active {
    background: var(--dropdown-hover-bg);
    color: var(--text-bright);
    font-weight: 600;
  }
</style>
