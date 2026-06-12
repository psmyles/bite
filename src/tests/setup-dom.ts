// Setup for the jsdom ('dom') vitest project — adds jest-dom matchers and
// unmounts components between tests so the DOM doesn't leak across cases.
import '@testing-library/jest-dom/vitest';
import { afterEach } from 'vitest';
import { cleanup } from '@testing-library/svelte';

afterEach(() => cleanup());
