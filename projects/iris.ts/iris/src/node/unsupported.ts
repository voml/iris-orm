const message =
    "@yydb/iris/node is only available on Node.js. Use @yydb/iris for browser hosts.";

/** Throws when a non-Node resolver loads the `/node` default export. */
export function createIris(): never {
    throw new Error(message);
}

export function loadProject(): never {
    throw new Error(message);
}

export function createIrisCli(): never {
    throw new Error(message);
}
