import { createRequire } from "node:module";

import { IrisFacadeError } from "../types/errors.ts";
import type { IrisNativeModule } from "./native-module.ts";
import { isOptionalPackageInstalled } from "./package-probe.ts";

export type NativeBinding = {
    readonly packageName: string;
    readonly platform: NodeJS.Platform;
    readonly arch: NodeJS.Arch;
    readonly module: IrisNativeModule;
};

const require = createRequire(import.meta.url);

let cached: NativeBinding | null = null;

/** Resolve the optional platform package name for the current host. */
export function resolvePlatformPackageName(
    platform: NodeJS.Platform = process.platform,
    arch: NodeJS.Arch = process.arch,
): string | null {
    if (platform === "win32" && arch === "x64") {
        return "@yydb/iris-win32-x64";
    }
    if (platform === "linux" && arch === "x64") {
        return "@yydb/iris-linux-x64";
    }
    return null;
}

function loadModuleFromPackage(packageName: string): IrisNativeModule {
    try {
        return require(packageName) as IrisNativeModule;
    } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        throw new IrisFacadeError(
            "native-load-failed",
            `@yydb/iris/node: failed to load ${packageName}: ${message}`,
        );
    }
}

/**
 * Load the native `.node` binding from the optional platform package.
 * Honors `NAPI_RS_NATIVE_LIBRARY_PATH` for local dev without publishing.
 */
export async function loadNativeBinding(): Promise<NativeBinding> {
    if (cached) {
        return cached;
    }

    const platform = process.platform;
    const arch = process.arch;
    const overridePath = process.env.NAPI_RS_NATIVE_LIBRARY_PATH;

    if (overridePath) {
        const module = loadModuleFromPackage(overridePath);
        cached = {
            packageName: overridePath,
            platform,
            arch,
            module,
        };
        return cached;
    }

    const packageName = resolvePlatformPackageName(platform, arch);
    if (!packageName) {
        throw new IrisFacadeError(
            "native-unsupported-platform",
            `@yydb/iris/node: no optional native package for ${platform}-${arch}`,
        );
    }
    if (!isOptionalPackageInstalled(packageName)) {
        throw new IrisFacadeError(
            "native-package-missing",
            `@yydb/iris/node: install optional dependency ${packageName} (same version as @yydb/iris)`,
        );
    }

    const module = loadModuleFromPackage(packageName);
    cached = {
        packageName,
        platform,
        arch,
        module,
    };
    return cached;
}

/** Reset cached binding (tests only). */
export function resetNativeBindingCacheForTests(): void {
    cached = null;
}
