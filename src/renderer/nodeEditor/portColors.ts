/** Visual color for each port data type — used on handles and edges.
 *  Values are read from CSS custom properties defined in theme.css.
 *  Lazy-initialized on first call so the stylesheet is guaranteed to
 *  have been applied before the map is built.                       */

function cssColor(prop: string): string {
  return getComputedStyle(document.documentElement).getPropertyValue(prop).trim()
}

let colorMap: Record<string, string> | null = null

function getMap(): Record<string, string> {
  if (colorMap) return colorMap
  colorMap = {
    image:   cssColor('--port-color-image'),
    mask:    cssColor('--port-color-mask'),
    number:  cssColor('--port-color-number'),
    string:  cssColor('--port-color-string'),
    boolean: cssColor('--port-color-boolean'),
    color:   cssColor('--port-color-color'),
    vector2: cssColor('--port-color-vector2'),
    vector3: cssColor('--port-color-vector3'),
    vector4: cssColor('--port-color-vector4'),
    numeric: cssColor('--port-color-numeric'),
    any:     cssColor('--port-color-any'),
    path:    cssColor('--port-color-path'),
  }
  return colorMap
}

export function portColor(type: string): string {
  const map = getMap()
  return map[type] ?? map.any ?? '#6b7280'
}
