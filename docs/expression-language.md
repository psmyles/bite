# BITE expression language v2

Expressions are parsed and checked at definition load. An `Expression` stores an
AST and its metadata dependencies; evaluation uses only the supplied context.
Statements, assignments, imports, filesystem, processes, time and randomness are
unavailable. Limits are 4,096 source bytes, 64 AST levels, 32 argument-group levels,
65,536 bytes per string result, and one to four vector components.

Operators: `+ - * / % == != < <= > >= && || !`. Precedence follows ordinary
arithmetic; boolean operators and `if` evaluate lazily. Use single or double
quoted strings; escapes are newline, carriage return, tab, quote and backslash.
Numbers support decimals and exponents. Vectors expose `.x/.y/.z/.w`; absent
components read as zero. Structured parameters cannot enter expression contexts.

Numeric operations broadcast. Vector/vector results have the left vector's
length; missing right components are zero for addition, subtraction and division,
one for multiplication, and the left component for interpolation. Division and
remainder by zero return zero. Ordered numeric comparisons use vector magnitude.
`round` follows JavaScript's ties toward positive infinity. Nonfinite computed
results retain IEEE behavior; golden JSON represents them explicitly.

Built-ins: `abs min max clamp floor ceil round sqrt pow sin cos tan lerp str int
float bool format if length normalize dot vec2 vec3 vec4 rgba`. `format` replaces
each literal `{}` with one argument and requires an exact placeholder count.
`str(vector)` returns comma-separated components. `rgba` passes color strings
through or converts four normalized components to `rgba(R,G,B,A)` with rounded
0–255 RGB and clamped 0–1 alpha. A resolved expression always contributes exactly
one process argument, including an empty string. Use `when` for omission.

Additional functions justified by shipped definitions:

- Text Filter: `lower`, `starts_with`, `ends_with`, `contains`.
- Properties: `strip_extension`, lexical `dirname`, `power_of_two`.
- EXIF properties: `rational` and `parse_int` preserve the legacy metadata parsing.

Metadata names are explicit `image.path`, `image.name`, `image.extension`,
`image.format`, `image.size`, `image.width`, `image.height`, `image.bit_depth`,
`image.dpi_x`, `image.dpi_y`, and the supported `image.exif.<tag>` strings.
The compiled metadata union includes all conditional branches.

## Reference coverage

The Rust definition tests compare 591 node argument/output cases and valid format
cases. Missing raw v1 parameters resolve declared v2 defaults; invalid enum/type
cases assert rejection. These explicit policies are recorded in
`tests/golden/KNOWN_DEVIATIONS.md`, while original reference cases remain intact.
Native executors are classified here and implemented in `bite-core`.
