/** Facade-level error (loaders, CLI, host mismatch). */
export class IrisFacadeError extends Error {
    readonly code: string;

    constructor(code: string, message: string) {
        super(message);
        this.name = "IrisFacadeError";
        this.code = code;
    }
}
