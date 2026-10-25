/** Language-layer error (WASM loading, host mismatch). Web hosts own this type. */
export class IrisLanguageError extends Error {
    readonly code: string;

    constructor(code: string, message: string) {
        super(message);
        this.name = "IrisLanguageError";
        this.code = code;
    }
}
