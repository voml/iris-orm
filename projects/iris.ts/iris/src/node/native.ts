/**
 * Node N-API loader — selects optional `@yydb/iris-{platform}-{arch}` packages.
 * Not imported from shared or web modules.
 */

export type NativeBinding = {
    readonly platform: NodeJS.Platform;
    readonly arch: NodeJS.Arch["arch"];
};

/** Resolve the optional platform package name for the current host. */
export function resolvePlatformPackageName(
    platform: NodeJS.Platform = process.platform,
    arch: NodeJS.Arch["arch"] = process.arch,
): string | null {
    if (platform === "win32" && arch === "x64") {
        return "@yydb/iris-win32-x64";
    }
    if (platform === "linux" && arch === "x64") {
        return "@yydb/iris-linux-x64";
    }
    return null;
}

/** Load the native `.node` binding when platform packages are published. */
export async function loadNativeBinding(): Promise<NativeBinding> {
    const packageName = resolvePlatformPackageName();
    if (!packageName) {
        throw new Error(
            `@yydb/iris/node: no optional native package for ${process.platform}-${process.arch}`,
        );
    }
    throw new Error(
        `@yydb/iris/node: native binding not implemented yet (expected optional ${packageName})`,
    );
}
