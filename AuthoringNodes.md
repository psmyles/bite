# Node Authoring Guide

Nodes are described by plain JSON files in this folder. The app loads them at startup and hot-reloads them automatically during development — no recompile, no restart required. Drop a new `.json` file here and it appears in the Node Library immediately.

---

## Table of Contents

1. [Quickstart — minimal working node](#1-quickstart--minimal-working-node)
2. [Top-level fields reference](#2-top-level-fields-reference)
3. [Port definitions (`inputs` / `outputs`)](#3-port-definitions-inputs--outputs)
4. [Param definitions (`params`)](#4-param-definitions-params)
5. [command_template — simple ImageMagick nodes](#5-command_template--simple-imagemagick-nodes)
6. [command_js — conditional ImageMagick args](#6-command_js--conditional-imagemagick-args)
7. [compute_js — pure-value nodes (no ImageMagick)](#7-compute_js--pure-value-nodes-no-imagemagick)
8. [needs_image_meta — per-image metadata](#8-needs_image_meta--per-image-metadata)
9. [params_visibility — show/hide Inspector rows](#9-params_visibility--showhide-inspector-rows)
10. [Worked examples](#10-worked-examples)
11. [Limitations and known constraints](#11-limitations-and-known-constraints)

---

## 1. Quickstart — minimal working node

The smallest possible image-processing node needs six fields:

```json
{
  "id": "auto_orient",
  "label": "Auto Orient",
  "category": "Transform",
  "inputs":  [{ "type": "image", "label": "Input" }],
  "outputs": [{ "type": "image", "label": "Output" }],
  "params": [],
  "command_template": "-auto-orient"
}
```

Save it as `node-definitions/auto-orient.json` and it will appear in the **Transform** section of the Node Library. Connecting it in the graph and clicking preview runs `magick <input> -auto-orient <output>`.

---

## 2. Top-level fields reference

| Field | Type | Required | Description |
|---|---|---|---|
| `id` | string | **yes** | Unique identifier. Snake_case. Never reuse an ID — the executor router, CLI exporter, and graph files all key on it. |
| `version` | string | no | Semantic version string (e.g. `"1.0.0"`). Not enforced, but useful for tracking breaking changes in saved workflows. |
| `label` | string | **yes** | Display name shown in the Node Library and on the node card header. |
| `description` | string | no | Tooltip text shown on a 1-second hover of the node header. Keep it to one sentence. |
| `category` | string | **yes** | Node Library group. Any string is valid — a new string creates a new category. Existing categories: `Transform`, `Color`, `Filters`, `Channels`, `Format`, `Output`, `Logic`, `Properties`, `Values`, `Math`, `Vector`, `Graph`. |
| `icon` | string | no | Lucide icon name for the node card header. Supported values: `blur`, `circle-half`, `crop`, `droplets`, `file-output`, `flip-horizontal`, `palette`, `resize`, `rotate-cw`, `sliders`, `sun`, `zap`. Omit to show no icon. |
| `inputs` | PortDefinition[] | **yes** | Upstream image/path ports on the left side. Use `[]` for source nodes (no image input). |
| `outputs` | PortDefinition[] | **yes** | Downstream image/path ports on the right side. Use `[]` for pure-value nodes. |
| `params` | ParamDefinition[] | **yes** | Inspector and wire-connectable parameters. Can be `[]`. |
| `command_template` | string | one of these | ImageMagick argument string with `{{param}}` placeholders. Use for simple nodes. |
| `command_js` | string | one of these | JS function body returning `string[]` of IM args. Use when args depend on logic. |
| `executor` | string | one of these | Key for a hardcoded TypeScript executor (reserved for built-in complex nodes). |
| `compute_js` | string | no | JS function body returning `Record<string, unknown>` of output values. Pure-value nodes only (no image ports). Alternative to `executor` for custom math/logic. |
| `needs_image_meta` | boolean | no | Set `true` to receive per-image metadata (dimensions, name, EXIF, etc.) in `compute_js`. Only meaningful for pure-value nodes that read file properties. |
| `params_visibility` | VisibilityRule[] | no | Inspector show/hide rules — hides a param row when another param has a specific value. |

**Exactly one** of `command_template`, `command_js`, or `executor` must be present on any image node.
Pure-value nodes (empty `inputs` and `outputs`) use `executor` or `compute_js` — not `command_template` / `command_js`.

---

## 3. Port definitions (`inputs` / `outputs`)

Each entry in `inputs` and `outputs` is an object with two fields:

```json
{ "type": "image", "label": "Input" }
```

### Port types

| Type | Color | Use for |
|---|---|---|
| `image` | Orange `#ff8c3f` | Pixel data — the main image pipeline |
| `mask` | Purple `#d8a4fc` | Grayscale mask (treated as image internally) |
| `path` | Yellow-green | File system path string |
| `number` | Cyan `#22d3ee` | Numeric values (rarely used as image ports) |

Most nodes use `image` for both input and output. `mask` is useful for nodes that process a channel as a grayscale image. `path` is used by the Folder Path node.

The `label` string appears on the port row inside the node card. Keep it short (`"Input"`, `"Output"`, `"R"`, `"Mask"`).

### Multiple outputs

Nodes can have more than one output port. Each gets its own handle, indexed from `out-0`:

```json
"outputs": [
  { "type": "image", "label": "R" },
  { "type": "image", "label": "G" },
  { "type": "image", "label": "B" },
  { "type": "image", "label": "A" }
]
```

This creates four handles (`out-0` through `out-3`). Multi-output nodes require a hardcoded TypeScript executor — `command_template` and `command_js` produce a single output only.

---

## 4. Param definitions (`params`)

Each entry in the `params` array describes one parameter: its data type, the Inspector widget used to edit it, its default value, and how it appears as a wire handle.

### Full param field reference

| Field | Type | Description |
|---|---|---|
| `name` | string | Internal identifier. Snake_case. Becomes the `{{name}}` placeholder in `command_template` and the key in `params` passed to `command_js` / `compute_js`. |
| `label` | string | Display name shown in the Inspector and on the port row. |
| `type` | ParamType | Data type — controls wire color and type-checking. See type table below. |
| `widget` | WidgetType | Inspector control. See widget table below. Can be omitted when `readonly: true` and `portOnly: true` (display-only port). |
| `default` | number \| string \| boolean \| number[] | Initial value when the node is placed. Must be compatible with `type`. |
| `min` | number | Lower bound for numeric types. Enforced by `slider` and `number` widgets. |
| `max` | number | Upper bound for numeric types. |
| `step` | number | Increment step for `slider` and `number` widgets. Defaults to `1` for `int`, `0.01` for `float`. |
| `options` | string[] | Required for `enum` type. The dropdown option list. The stored value is one of these strings. |
| `readonly` | boolean | If `true`: right-side output handle only; no input handle. In the Inspector the widget is display-only (shows the value but the user cannot edit it). Use for computed outputs like `result`. |
| `portOnly` | boolean | If `true`: the param is rendered only as a right-side port handle — no row in the node body, no Inspector row. Use for derived multi-output params (e.g. RGBA component outputs). Implies `readonly`. |
| `noPort` | boolean | If `true`: Inspector row and widget only — no wire handle at all. Use for params that are UI-only (e.g. a color picker that controls the node's display but is never wired). |

### Param types

| Type | Stored as | Wire color | Notes |
|---|---|---|---|
| `int` | integer number | Cyan | Parsed with `parseInt`. Slider snaps to whole numbers. |
| `float` | decimal number | Cyan | Parsed with `parseFloat`. |
| `numeric` | number or number[] | Near-white | Accepts int, float, vector, or color — use for math nodes that should accept anything numeric. |
| `string` | string | Green | Plain text. |
| `enum` | string | Green | One of the `options` values. Always use a `dropdown` widget. |
| `bool` | boolean | Yellow | `true` / `false`. Use a `checkbox` widget. |
| `color` | `"#rrggbb"` string or `[r,g,b,a]` array | Pink | Use `"#rrggbb"` default for `color-picker` widget; use `[r,g,b,a]` (0–1 floats) default for `vector` widget. |
| `vector2` | `[x, y]` | Amber | 2-component float array. |
| `vector3` | `[x, y, z]` | Indigo | 3-component float array. |
| `vector4` | `[x, y, z, w]` | Teal | 4-component float array. |
| `any` | any | Slate | Accepts any wired value without type-checking. Use for nodes that compare or forward arbitrary values. |

### Widget types

| Widget | Suitable types | Description |
|---|---|---|
| `slider` | `int`, `float`, `numeric` | Horizontal drag slider. Requires `min` and `max`. |
| `number` | `int`, `float`, `numeric` | Numeric input box. `min`, `max`, and `step` are advisory. |
| `dropdown` | `enum` | Select from the `options` list. Requires `options`. |
| `text` | `string` | Single-line text input. |
| `checkbox` | `bool` | Toggle on/off. |
| `color-picker` | `color` | HSL/hex color picker. Default should be a `"#rrggbb"` string. |
| `vector` | `vector2`, `vector3`, `vector4`, `color` | Row of N numeric inputs. Default should be a number array. |

---

## 5. `command_template` — simple ImageMagick nodes

`command_template` is a string of ImageMagick arguments with `{{param_name}}` placeholders. On execution, each placeholder is replaced by the current (or default) value of the matching param, then the whole string is split on whitespace and appended to the `magick` invocation.

```
magick <input_file>  <command_template args>  <output_file>
```

### Rules

- **One template per node.** The template produces all arguments for the node. It cannot branch or loop.
- **Whitespace splitting.** The interpolated string is split on one or more spaces. The result is a flat `string[]` — no shell quoting is performed. This means multi-word values (e.g. filter names like `"Mitchell Netravali"`) would be split into two arguments. Avoid param values with embedded spaces; prefer hyphenated names.
- **Fallback to default.** If a param name has no current value, its `default` is used. If `default` is also absent, the placeholder becomes an empty string.
- **All values are stringified.** Numbers, booleans, and arrays are converted via `String()`. For arrays (vector/color types) this produces a comma-separated list — which is rarely what ImageMagick expects. Avoid using vector params directly in `command_template`; use `command_js` instead.
- **Literal `%` signs.** ImageMagick uses `%` as a format-string prefix. In templates representing percentage values, write the literal `%` directly: `"-level {{black_point}}%,{{white_point}}%"`. The shell is not involved — no escaping needed beyond what ImageMagick expects.

### Examples

| Template | ImageMagick docs | Effect |
|---|---|---|
| `"-blur 0x{{sigma}}"` | `-blur radius×sigma` | Gaussian blur |
| `"-brightness-contrast {{brightness}}x{{contrast}}"` | `-brightness-contrast` | Brightness/contrast |
| `"-level {{black}}%,{{white}}%,{{gamma}}"` | `-level` | Black/white point + gamma |
| `"-crop {{w}}x{{h}}+{{x}}+{{y}} +repage"` | `-crop` | Crop + reset canvas origin |
| `"-background '{{color}}' -rotate {{angle}}"` | `-rotate` | Rotate with background fill |
| `"-modulate 100,100,{{hue}}"` | `-modulate` | Hue shift (modulate takes brightness,saturation,hue) |

---

## 6. `command_js` — conditional ImageMagick args

When the arguments depend on logic that `command_template`'s dumb substitution cannot express — conditional flags, computed geometry, string concatenation — use `command_js` instead.

`command_js` is the **body** of a JavaScript function. The function receives a single argument named `params` (a `Record<string, unknown>` of the fully-resolved param values) and **must return `string[]`** — a flat array of ImageMagick argument tokens, one token per string, ready to spread into `spawn()`.

```json
"command_js": "return ['-some-flag', String(params.value)]"
```

The sandbox is minimal: only `params` is in scope. No `require`, no `fs`, no globals. Standard JS operators, `Math`, `String`, `Number`, `Array`, template literals, and destructuring all work.

### When to use `command_js` instead of `command_template`

| Situation | Use |
|---|---|
| Fixed argument string, params only substituted in | `command_template` |
| A flag is present only when a boolean param is true | `command_js` |
| Two params need to be concatenated without a space | `command_js` |
| Argument value requires arithmetic on a param | `command_js` |
| An enum param maps to a non-trivial IM argument | `command_js` |
| A vector or color param needs to be formatted | `command_js` |

### Return value requirements

- Must be `string[]` — an array where every element is a string.
- Each element is one whitespace-separated token as you would write it in a shell. Do **not** include shell quotes; they are not interpreted. Write `"50x50"` not `'"50x50"'`.
- Return `[]` to emit no arguments (pass-through with no operation).
- Throwing an error cancels processing for that image and logs the error to the console.

---

## 7. `compute_js` — pure-value nodes (no ImageMagick)

Pure-value nodes have empty `inputs: []` and `outputs: []` arrays. They perform computation in JavaScript (no ImageMagick spawning) and expose their outputs via `readonly` params connected to downstream nodes via param-wires.

`compute_js` is the **body** of a JavaScript function. It receives `params` (the current resolved param values) and **must return a plain `Record<string, unknown>`** — an object mapping param names to their new output values. The returned values are merged back into `params` and forwarded to downstream wires.

```json
"compute_js": "return { result: params.a + params.b }"
```

The same sandbox restrictions as `command_js` apply: only `params` in scope, no Node.js APIs.

### When to use `compute_js`

Use it for any custom pure math or logic that the built-in executor keys (`math_add`, `logic_and`, etc.) don't cover. Examples: custom clamping, percentage calculation, string formatting, lookup table mapping.

### Return value requirements

- Must be a plain object (`{}`). Arrays and primitives are rejected.
- Keys must match param `name` values of `readonly` params — those are the output ports that downstream nodes can wire to.
- You do not need to include non-output params in the return value; the engine merges your return with the existing `params`.

---

## 8. `needs_image_meta` — per-image metadata

For pure-value nodes that read per-image properties (like the built-in Properties nodes), set:

```json
"needs_image_meta": true
```

This opts the node into the per-image metadata loading path. Without this flag, `compute_js` receives only the static param values from the Inspector — no image information.

**What metadata is available via `needs_image_meta`:**
The built-in Properties nodes cover all available metadata fields: file name, file path, pixel dimensions (width/height), file size in bytes, bit depth, file type/format, DPI resolution, and raw EXIF tags. Custom `compute_js` nodes with `needs_image_meta: true` currently receive static params only — metadata fields are not yet injected into the `params` object for custom nodes. This flag primarily prevents the executor from skipping the `magick identify` call, which is a prerequisite for any future metadata injection.

> **Practical advice:** If you need per-image metadata in a custom node today, chain it after a built-in Properties node (Name, Dimensions, etc.) and wire the output value to your node's input param. That is simpler than building a custom `needs_image_meta` node.

---

## 9. `params_visibility` — show/hide Inspector rows

`params_visibility` is an array of rules that control which param rows are visible in the Inspector depending on the current value of another param. This is useful for showing advanced options only when a particular mode is selected.

```json
"params_visibility": [
  { "show": "custom_value", "when": { "param": "mode", "eq": "custom" } }
]
```

Each rule object has:

| Field | Description |
|---|---|
| `show` | The `name` of the param whose Inspector row this rule controls. |
| `when.param` | The `name` of the controlling param. |
| `when.eq` | The value the controlling param must equal for the row to be visible. Strict equality (`===`). |

Rules are applied only in the Inspector — they do not affect port handles or `command_template` substitution. A param hidden by a visibility rule still participates in command building using its current (or default) value.

A param with no matching rule is always visible. Only one rule per `show` name is evaluated — the first match wins.

---

## 10. Worked examples

### Example A — Posterize (simple `command_template`)

Posterize reduces each channel to a fixed number of distinct levels, creating a flat graphic art effect. The ImageMagick argument is `-posterize <levels>`.

```json
{
  "id": "posterize",
  "version": "1.0.0",
  "label": "Posterize",
  "description": "Reduce each colour channel to N discrete levels",
  "category": "Color",
  "icon": "circle-half",
  "inputs":  [{ "type": "image", "label": "Input" }],
  "outputs": [{ "type": "image", "label": "Output" }],
  "params": [
    {
      "name":    "levels",
      "label":   "Levels",
      "type":    "int",
      "widget":  "slider",
      "default": 4,
      "min":     2,
      "max":     256
    }
  ],
  "command_template": "-posterize {{levels}}"
}
```

**What ImageMagick runs:**
```
magick input.png  -posterize 4  output.png
```

**Why `command_template` is sufficient here:** the argument is a single flag followed by a single substituted integer. No conditional logic, no concatenation.

---

### Example B — Add Noise (`command_js` with enum → conditional arg)

ImageMagick's noise injection uses two arguments: an optional `-attenuate <amount>` to scale the intensity, followed by `+noise <Type>` where `Type` is one of `Gaussian`, `Impulse`, `Laplacian`, `Multiplicative`, `Poisson`, `Random`, or `Uniform`.

The `-attenuate` flag must only be emitted when `attenuate` differs from the default of 1 (otherwise it's redundant), and the noise type string must be capitalised exactly. Neither of these can be expressed in `command_template`.

```json
{
  "id": "add_noise",
  "version": "1.0.0",
  "label": "Add Noise",
  "description": "Inject random noise into the image",
  "category": "Filters",
  "inputs":  [{ "type": "image", "label": "Input" }],
  "outputs": [{ "type": "image", "label": "Output" }],
  "params": [
    {
      "name":    "noise_type",
      "label":   "Type",
      "type":    "enum",
      "widget":  "dropdown",
      "default": "Gaussian",
      "options": ["Gaussian", "Impulse", "Laplacian", "Multiplicative", "Poisson", "Random", "Uniform"]
    },
    {
      "name":    "attenuate",
      "label":   "Attenuate",
      "type":    "float",
      "widget":  "slider",
      "default": 1,
      "min":     0.1,
      "max":     5,
      "step":    0.1
    }
  ],
  "command_js": "const args = []; if (params.attenuate !== 1) { args.push('-attenuate', String(params.attenuate)); } args.push('+noise', String(params.noise_type)); return args;"
}
```

**What ImageMagick runs (attenuate = 1, Gaussian):**
```
magick input.png  +noise Gaussian  output.png
```

**What ImageMagick runs (attenuate = 2.5, Impulse):**
```
magick input.png  -attenuate 2.5  +noise Impulse  output.png
```

The `command_js` body expanded for readability:

```js
const args = []

// Only emit -attenuate when it differs from the neutral value of 1
if (params.attenuate !== 1) {
  args.push('-attenuate', String(params.attenuate))
}

args.push('+noise', String(params.noise_type))
return args
```

---

### Example C — Remap Range (`compute_js` pure-value node)

This node linearly remaps an input value from one numeric range to another — for example, turning a 0–255 byte value into a 0–1 normalised float, or mapping a 0–1 value to a custom decibel range. No ImageMagick involvement.

```json
{
  "id": "remap_range",
  "version": "1.0.0",
  "label": "Remap Range",
  "description": "Linearly remap a value from [in_min, in_max] to [out_min, out_max]",
  "category": "Math",
  "inputs":  [],
  "outputs": [],
  "params": [
    {
      "name":    "value",
      "label":   "Value",
      "type":    "numeric",
      "widget":  "number",
      "default": 0
    },
    {
      "name":    "in_min",
      "label":   "In Min",
      "type":    "float",
      "widget":  "number",
      "default": 0
    },
    {
      "name":    "in_max",
      "label":   "In Max",
      "type":    "float",
      "widget":  "number",
      "default": 255
    },
    {
      "name":    "out_min",
      "label":   "Out Min",
      "type":    "float",
      "widget":  "number",
      "default": 0
    },
    {
      "name":    "out_max",
      "label":   "Out Max",
      "type":    "float",
      "widget":  "number",
      "default": 1
    },
    {
      "name":    "result",
      "label":   "Result",
      "type":    "numeric",
      "widget":  "number",
      "default": 0,
      "readonly": true
    }
  ],
  "compute_js": "const range = params.in_max - params.in_min; const t = range === 0 ? 0 : (Number(params.value) - params.in_min) / range; return { result: params.out_min + t * (params.out_max - params.out_min) };"
}
```

The `compute_js` body expanded:

```js
const range = params.in_max - params.in_min

// Guard against division by zero when in_min === in_max
const t = range === 0 ? 0 : (Number(params.value) - params.in_min) / range

return { result: params.out_min + t * (params.out_max - params.out_min) }
```

The `value` param has no `readonly` or `portOnly` flag, so it appears as an **input** wire handle on the left side (consumers can wire e.g. a Dimensions node's `width` output directly in) and as an Inspector number box. `result` is `readonly`, so it appears as an **output** handle on the right side and is display-only in the Inspector.

---

### Example D — Adaptive Resize with `params_visibility`

Demonstrates hiding a param row depending on another param's value. When `mode` is `"pixels"` the `scale` slider is hidden; when `mode` is `"percent"` the `pixels` input is hidden.

```json
{
  "id": "adaptive_resize",
  "version": "1.0.0",
  "label": "Adaptive Resize",
  "description": "Content-aware resize that avoids distorting important areas",
  "category": "Transform",
  "inputs":  [{ "type": "image", "label": "Input" }],
  "outputs": [{ "type": "image", "label": "Output" }],
  "params": [
    {
      "name":    "mode",
      "label":   "Mode",
      "type":    "enum",
      "widget":  "dropdown",
      "default": "pixels",
      "options": ["pixels", "percent"]
    },
    {
      "name":    "pixels",
      "label":   "Size (px)",
      "type":    "int",
      "widget":  "number",
      "default": 512,
      "min":     1,
      "max":     16384
    },
    {
      "name":    "scale",
      "label":   "Scale (%)",
      "type":    "float",
      "widget":  "slider",
      "default": 50,
      "min":     1,
      "max":     400,
      "step":    1
    }
  ],
  "params_visibility": [
    { "show": "pixels", "when": { "param": "mode", "eq": "pixels" } },
    { "show": "scale",  "when": { "param": "mode", "eq": "percent" } }
  ],
  "command_js": "if (params.mode === 'percent') { return ['-adaptive-resize', params.scale + '%']; } return ['-adaptive-resize', String(params.pixels)];"
}
```

When the user selects **percent** in the dropdown, the **Size (px)** row disappears and the **Scale (%)** slider appears. The hidden row's value is still substituted in the command (using its current or default value) — visibility rules only affect the Inspector display.

---

## 11. Limitations and known constraints

### `command_template` cannot

- Emit zero arguments conditionally (always produces at least the interpolated string)
- Format multi-component values (vector/color arrays stringify as `"1,0,0,1"` which ImageMagick does not accept as a geometry or color)
- Compute derived values (e.g. `half_width = width / 2`)
- Apply conditional flags based on param values
- Emit more than one logically separate IM operation (multiple `-flag value` pairs are fine since they're all part of the same linear chain, but you cannot branch)

Use `command_js` for any of the above.

### `command_js` / `compute_js` sandbox

- Only `params` is in scope. No Node.js modules (`fs`, `path`, `child_process`, etc.).
- Standard JS globals are available: `Math`, `Number`, `String`, `Array`, `Object`, `JSON`, `parseInt`, `parseFloat`, template literals, destructuring.
- Do not rely on `this` — the function is called as a plain function, not a method.
- Errors thrown inside the body propagate as processing errors for that image. The batch continues; the failed image is counted in the error summary.
- The body is a string stored in the JSON file, evaluated once per image via `new Function('params', body)`. Keep it fast — it runs for every image in the batch.

### Multi-output nodes require a TypeScript executor

`command_template` and `command_js` always produce exactly one output image. To split into multiple image outputs (like Channel Split's R/G/B/A outputs), a hardcoded executor key in `executor.ts` is required. This is not something a JSON-only node can do.

### No dynamic port counts

The number of `inputs`, `outputs`, and `params` is fixed at load time. You cannot have a variable-port node (e.g. "merge N images") from JSON alone.

### `executor` keys are reserved

The following `executor` values are handled by hardcoded branches in `executor.ts` and `executor-compute.ts` and must not be used in new nodes unless you are modifying the TypeScript source:

`resize`, `gate`, `rename`, `channel_split`, `channel_merge`, `mean_value`, `solid_image`, `format_convert`, `math_add`, `math_subtract`, `math_multiply`, `math_divide`, `math_power`, `math_lerp`, `logic_and`, `logic_or`, `logic_not`, `logic_branch`, `logic_comparison`, `text_filter`, `split_vec`, `append_vec`, `vec_math_dot`, `vec_math_length`, `vec_math_normalize`, `value_color`, `prop_name`, `prop_path`, `prop_dimensions`, `prop_size`, `prop_bitdepth`, `prop_filetype`, `prop_resolution`, `prop_exif`, `prop_power_of_two`

For new custom logic, use `command_js` (image nodes) or `compute_js` (pure-value nodes) instead.
