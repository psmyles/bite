// Built-in executor registrations for standard image nodes.
// Imported once at executor.ts module load time for its side effects.
//
// To add a new executor without touching executor.ts:
//   1. Add your node JSON to node-definitions/ with "executor": "my_op"
//   2. Register the arg builder here: registerExecutor('my_op', (def, params) => [...])
//
// Nodes with complex multi-stream or pipeline-flow semantics (channel_split,
// channel_merge, mean_value, format_convert, gate, rename) are handled as
// hardcoded branches inside executor.ts because they require access to internal
// pipeline state (image buffer maps, caching, output format tracking) that goes
// beyond what ArgBuilderFn can express.

import { registerExecutor } from './executorRegistry.js'
import { buildResizeArgs } from './executor-compute.js'

registerExecutor('resize', (_def, params) => buildResizeArgs(params))
