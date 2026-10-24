/** Facade-level error (loaders, CLI, host mismatch). */
export class IrisFacadeError extends Error {
    readonly code: string;

    constructor(code: string, message: string) {
        super(message);
        this.name = "IrisFacadeError";
        this.code = code;
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
