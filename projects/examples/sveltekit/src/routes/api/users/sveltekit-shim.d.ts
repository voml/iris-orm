declare module "./$types" {
    import type { RequestEvent } from "@sveltejs/kit";

    export interface RequestHandler {
        (event: RequestEvent): Response | Promise<Response>;
    }
}
