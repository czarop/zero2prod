-- Add migration script here
INSERT INTO users (user_id, username, password_hash)
VALUES (
'9500a7c3-55fe-4d59-94b2-81078bf44d34',
'admin',
'$argon2id$v=19$m=15000,t=2,p=1$yIREZ68kBGBFLLyeeUVIAg$MYZv4UzZY7MKvufyyU/4q1wghKzDWsAhR4dwAUscrSw'
);