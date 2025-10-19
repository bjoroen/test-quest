INSERT INTO users (id, name) VALUES
    (1, 'Alice'),
    (2, 'Bob'),
    (3, 'Charlie')
ON CONFLICT (id) DO NOTHING;
