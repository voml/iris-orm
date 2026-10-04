import { register } from "node:module";

register(new URL("./generated-e2e-resolver.mjs", import.meta.url).href, import.meta.url);
