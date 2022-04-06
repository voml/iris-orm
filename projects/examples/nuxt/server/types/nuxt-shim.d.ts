declare function defineEventHandler<T extends (...args: never[]) => unknown>(handler: T): T;
declare function readBody<T>(event: unknown): Promise<T>;
declare function getQuery(event: unknown): Record<string, string | string[] | undefined>;
declare function createError(input: { statusCode: number; statusMessage?: string }): Error;
declare function defineNuxtConfig(config: Record<string, unknown>): Record<string, unknown>;
