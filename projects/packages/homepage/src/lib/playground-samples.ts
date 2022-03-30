export const SAMPLE_SCHEMA = `table User {
    @@user_id: utf8,
    @user_name: utf8,
    active: bool,
}`;

export const SAMPLE_DML = `User.filter(x => x.active).collect()`;
