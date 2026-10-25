export const USER_SCHEMA = `
table User {
    @@user_id: utf8,
    @user_name: utf8,
    active: bool,
}
`;

export const USER_WIRE_NAMES = {
    User: { userId: "user_id", userName: "user_name", active: "active" },
} as const;

export const INVALID_SCHEMA = "not a schema";

export function srcImport(relativeFromPkgRoot: string): string {
    return new URL(relativeFromPkgRoot, new URL("../", import.meta.url)).href;
}

export function isWasmCoreMissing(error: unknown): boolean {
    return (
        error instanceof Error &&
        "code" in error &&
        (error.code === "wasm-package-missing" || error.message.includes("iris_wasm"))
    );
}
