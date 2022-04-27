import type { IrisBindings } from "../bindings.ts";
import { IrisFacadeError } from "../types/errors.ts";

let cached: Promise<IrisBindings> | undefined;
let active: IrisBindings | null = null;

export function getCachedLoader(): Promise<IrisBindings> | undefined {
    return cached;
}

export function setCachedLoader(loader: Promise<IrisBindings> | undefined): void {
    cached = loader;
}

export function setActiveBindings(bindings: IrisBindings | null): void {
    active = bindings;
}

/** Active semantic core after `loadIrisWeb()` / `initIris()`. */
export function getWasmSemanticCore(): IrisBindings {
    if (active == null) {
        throw new IrisFacadeError("wasm-not-initialized", "@yydb/iris: call initIris() or loadIrisWeb() before createIris()");
    }
    return active;
}

/** @internal Reset init gate (binding tests only). */
export function resetInitStateForTests(): void {
    cached = undefined;
    active = null;
}
