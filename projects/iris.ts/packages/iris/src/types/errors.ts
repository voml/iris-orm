/** Facade-level error (loaders, CLI, host mismatch, DSN open failures). */
export class IrisFacadeError extends Error {
    readonly code: string;
    /** Semantic kind of the author code, when the native envelope carries one. */
    readonly kind?: string;
    /** Internal diagnostic that never replaces the author code. */
    readonly diagnosticCode?: string;

    constructor(code: string, message: string, details: { kind?: string; diagnosticCode?: string } = {}) {
        super(message);
        this.name = "IrisFacadeError";
        this.code = code;
        if (details.kind !== undefined) {
            this.kind = details.kind;
        }
        if (details.diagnosticCode !== undefined) {
            this.diagnosticCode = details.diagnosticCode;
        }
    }
}

/** Structured project-config failure (code, args, config path, cause). */
export class IrisConfigError extends Error {
    readonly code: string;
    readonly args: Readonly<Record<string, string>>;
    readonly configPath: string;

    constructor(code: string, configPath: string, args: Readonly<Record<string, string>>, cause?: unknown) {
        super(args.reason ?? "project config failed", cause === undefined ? undefined : { cause });
        this.name = "IrisConfigError";
        this.code = code;
        this.args = args;
        this.configPath = configPath;
    }
}
