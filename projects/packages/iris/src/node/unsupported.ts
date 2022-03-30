import { IrisFacadeError } from "../types/errors.ts";

const HOST_MESSAGE = "@yydb/iris/node is only available on Node.js. Use @yydb/iris for browser hosts.";

function unsupported(): never {
    throw new IrisFacadeError("node-host-required", HOST_MESSAGE);
}

export const createIris = unsupported;
export const loadProject = unsupported;
export const checkSchemaFile = unsupported;
export const printDoctorReport = unsupported;
