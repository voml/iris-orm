export const USER_SCHEMA = `
table User {
    @@user_id: utf8,
    @user_name: utf8,
    active: bool,
}
`;

// Must match the fingerprint asserted by `iris-wasm/tests/check_source.rs`.
export const USER_SCHEMA_FINGERPRINT = "108401c5f275ffe6";

export const INVALID_SCHEMA = "not a schema";
