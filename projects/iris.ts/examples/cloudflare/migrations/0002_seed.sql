INSERT OR IGNORE INTO User (user_id, user_name, active) VALUES
    ('550e8400-e29b-41d4-a716-446655440000', 'ada', 1),
    ('6ba7b810-9dad-11d1-80b4-00c04fd430c8', 'linus', 1);

INSERT OR IGNORE INTO Post (post_id, author, title, published) VALUES
    ('11111111-1111-4111-8111-111111111101', '550e8400-e29b-41d4-a716-446655440000', 'Ada post', 1),
    ('22222222-2222-4222-8222-222222222202', '6ba7b810-9dad-11d1-80b4-00c04fd430c8', 'Linus post', 1);
