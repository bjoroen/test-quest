INSERT INTO users (id, name, password) VALUES
    (1, 'Alice', '123'),
    (2, 'Harry Potter', '1234'),
    (3, 'Charlie', '4321')
ON CONFLICT (id) DO NOTHING;
