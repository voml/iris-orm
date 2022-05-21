import blog from "../schemas/blog.iris?raw";
import seed from "../schemas/seed.iris?raw";

/** Merged schema text for WASM `Database.create({ schema })` inside the Worker isolate. */
export const IRIS_SCHEMA = `${blog}\n\n${seed}`;
