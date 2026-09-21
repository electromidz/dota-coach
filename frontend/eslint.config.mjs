import nextCoreWebVitals from "eslint-config-next/core-web-vitals";
import nextTypescript from "eslint-config-next/typescript";

/**
 * `eslint-config-next` 16 ships native flat configs, so they are imported
 * directly.
 *
 * This used to go through `FlatCompat`, which crashed before linting a single
 * file — its config validator `JSON.stringify`s the config to format an error,
 * and the plugin graph it is handed now contains a cycle. The translation layer
 * is unnecessary here: both entry points already export flat config arrays.
 */
const config = [
  ...nextCoreWebVitals,
  ...nextTypescript,
  { ignores: [".next/**", "node_modules/**"] },
];

export default config;
