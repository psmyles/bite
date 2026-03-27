import { vitePreprocess } from '@sveltejs/vite-plugin-svelte'

/** @type {import('@sveltejs/vite-plugin-svelte').SvelteConfig} */
export default {
  preprocess: vitePreprocess(),
  onwarn(warning, handler) {
    // Drag-handle divs are intentionally non-standard ARIA — suppress these
    if (
      warning.code === 'a11y_no_noninteractive_element_interactions' ||
      warning.code === 'a11y_no_noninteractive_tabindex' ||
      warning.code === 'a11y_no_static_element_interactions' ||
      warning.code === 'a11y_click_events_have_key_events'
    ) return
    handler(warning)
  },
}
